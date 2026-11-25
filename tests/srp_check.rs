//! Checks that the SRP proof our sign-in sends is what Apple's server
//! expects, by playing the server with an independent big-number library.
//! Apple GSA variant: RFC 5054, SHA-256, 2048-bit group, no username in x.

use num_bigint::BigUint;
use sha2::{Digest, Sha256};

const N_HEX: &str = "ac6bdb41324a9a9bf166de5e1389582faf72b6651987ee07fc3192943db56050a37329cbb4a099ed8193e0757767a13dd52312ab4b03310dcd7f48a9da04fd50e8083969edb767b0cf6095179a163ab3661a05fbd5faaae82918a9962f0b93b855f97993ec975eeaa80d740adbf4ff747359d041d5c33ea71d281e446b14773bca97b43a23fb801676bd207a436c6481f1d2b9078717461a5b9d32e688f87748544523b524b0d57d5ea77a2775d2ecfa032cfbdbf52fb3786160279004e57ae6af874e7303ce53299ccc041c7bc308d82a5698f3a8d0c38271ae35f8e9dbfbb694b5c803d89f7ae435de236d525f54759b65e372fcd68ef20fa7111f9e4aff73";

fn h(parts: &[&[u8]]) -> Vec<u8> {
    let mut d = Sha256::new();
    for p in parts {
        d.update(p);
    }
    d.finalize().to_vec()
}

fn pad(x: &BigUint, len: usize) -> Vec<u8> {
    let b = x.to_bytes_be();
    let mut v = vec![0u8; len - b.len()];
    v.extend_from_slice(&b);
    v
}

/// Simple deterministic byte generator so the test repeats exactly.
fn bytes(seed: u64, n: usize) -> Vec<u8> {
    let mut x = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    (0..n)
        .map(|_| {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            (x >> 24) as u8
        })
        .collect()
}

fn check_once(seed: u64, email: &str, password: &str) {
    let n = BigUint::parse_bytes(N_HEX.as_bytes(), 16).unwrap();
    let g = BigUint::from(2u32);
    let nlen = n.to_bytes_be().len();

    // what Apple stores: salt, iterations and the verifier
    let salt = bytes(seed, 16);
    let iters = 20_000u32;
    let mut p = [0u8; 32];
    let pw_hash = Sha256::digest(password.as_bytes());
    pbkdf2_sha256(&pw_hash, &salt, iters, &mut p);
    let x = BigUint::from_bytes_be(&h(&[&salt, &h(&[b":", &p])]));
    let v = g.modpow(&x, &n);

    // client step 1 (our build)
    let client = srp::Client::<srp::groups::G2048, sha2::Sha256>::new_with_options(false);
    let a = bytes(seed + 1, 32);
    let a_pub_bytes = client.compute_public_ephemeral(&a);
    let a_pub = BigUint::from_bytes_be(&a_pub_bytes);
    assert_eq!(a_pub, g.modpow(&BigUint::from_bytes_be(&a), &n), "A = g^a is wrong");

    // server step 1 (reference)
    let k = BigUint::from_bytes_be(&h(&[&n.to_bytes_be(), &pad(&g, nlen)]));
    let b = BigUint::from_bytes_be(&bytes(seed + 2, 32));
    let b_pub = (&k * &v + g.modpow(&b, &n)) % &n;
    let b_pub_bytes = b_pub.to_bytes_be();

    // client step 2 (our build)
    let verifier = client
        .process_reply(&a, email.as_bytes(), &p, &salt, &b_pub_bytes)
        .expect("process_reply failed");

    // server step 2 (reference): S = (A * v^u)^b
    let u = BigUint::from_bytes_be(&h(&[&pad(&a_pub, nlen), &pad(&b_pub, nlen)]));
    let s = (&a_pub * v.modpow(&u, &n)).modpow(&b, &n) % &n;
    let key = h(&[&s.to_bytes_be()]);
    let hn = h(&[&n.to_bytes_be()]);
    let hg = h(&[&pad(&g, nlen)]);
    let hxor: Vec<u8> = hn.iter().zip(hg.iter()).map(|(a, b)| a ^ b).collect();
    let m1 = h(&[&hxor, &h(&[email.as_bytes()]), &salt, &a_pub_bytes, &b_pub_bytes, &key]);

    assert_eq!(
        verifier.proof().to_vec(),
        m1,
        "M1 mismatch (seed {seed}): the server would reply 'wrong password'"
    );
}

fn pbkdf2_sha256(password: &[u8], salt: &[u8], iters: u32, out: &mut [u8; 32]) {
    // independent PBKDF2-HMAC-SHA256 (one block)
    fn hmac(key: &[u8], msg: &[u8]) -> Vec<u8> {
        let mut k = [0u8; 64];
        if key.len() > 64 {
            k[..32].copy_from_slice(&Sha256::digest(key));
        } else {
            k[..key.len()].copy_from_slice(key);
        }
        let ipad: Vec<u8> = k.iter().map(|b| b ^ 0x36).collect();
        let opad: Vec<u8> = k.iter().map(|b| b ^ 0x5c).collect();
        h(&[&opad, &h(&[&ipad, msg])])
    }
    let mut msg = salt.to_vec();
    msg.extend_from_slice(&1u32.to_be_bytes());
    let mut u = hmac(password, &msg);
    let mut t = u.clone();
    for _ in 1..iters {
        u = hmac(password, &u);
        for (a, b) in t.iter_mut().zip(u.iter()) {
            *a ^= b;
        }
    }
    out.copy_from_slice(&t);
}

#[test]
fn srp_proof_matches_reference() {
    for seed in 0..25u64 {
        check_once(seed * 977 + 3, "Someone.Name@icloud.com", "Correct horse 9!");
    }
}

#[test]
fn pbkdf2_matches_reference() {
    for (i, pw) in ["Correct horse 9!", "pässwörd€", "a"].iter().enumerate() {
        let salt = bytes(i as u64 + 50, 16);
        let pw_hash = Sha256::digest(pw.as_bytes());
        for proto_fo in [false, true] {
            let input: Vec<u8> = if proto_fo {
                pw_hash.iter().map(|b| format!("{b:02x}")).collect::<String>().into_bytes()
            } else {
                pw_hash.to_vec()
            };
            let mut ours = [0u8; 32];
            pbkdf2::pbkdf2::<hmac::Hmac<sha2::Sha256>>(&input, &salt, 1000, &mut ours).unwrap();
            let mut reference = [0u8; 32];
            pbkdf2_sha256(&input, &salt, 1000, &mut reference);
            assert_eq!(ours, reference, "PBKDF2 mismatch for case {i} fo={proto_fo}");
        }
    }
}
