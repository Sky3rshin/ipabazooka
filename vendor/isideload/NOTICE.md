# isideload (patched copy)

Copy of the `isideload` 0.5.2 crate by Nicholas Sharp (https://github.com/nab138/isideload),
MIT License, used by ipabazooka.

Changes made for ipabazooka:

- `src/anisette/mod.rs`: `AnisetteData::get_headers` now sends the full set of
  Apple identity headers (client time, time zone, locale, serial, routing info,
  local user ID) along with the machine ID, one-time password and device ID.
  Upstream sent only the last three, which can make Apple reply -22406
  ("Apple ID or password incorrect") even when the password is right.
- `src/anisette/mod.rs`: added `AnisetteData::from_parts` so ipabazooka can supply
  anisette data generated locally by Apple's iCloud for Windows (AOSKit).
- `src/auth/grandslam.rs`: removed an unneeded `mut`.
- `src/auth/grandslam.rs`: the HTTP client gives up connecting after 10 s and
  on any request after 90 s, instead of hanging on a dead connection. When the
  URL bag can't be fetched, the report keeps every underlying cause (DNS,
  connect, TLS...) through the new `error_chain` helper.
