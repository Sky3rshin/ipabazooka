// ipabazooka local anisette helper (32-bit).
//
// Asks Apple's own iCloud for Windows component (AOSKit.dll, installed by the
// web/"non-Store" version of iCloud) for this PC's sign-in identity data
// ("anisette": a machine ID and a one-time password). This is the same public
// technique AltServer for Windows has used. AOSKit is 32-bit only, so this
// helper is built for x86 and the 64-bit app runs it and reads its output.
//
// Output (stdout, one line of JSON):
//   {"md":"<one-time password>","mdm":"<machine id>","icloud":"<iCloud client info>"}
// or {"error":"<what went wrong>"} with exit code 1.

using System;
using System.IO;
using System.Runtime.InteropServices;
using System.Text;

static class AnisetteHelper
{
    [DllImport("kernel32", SetLastError = true, CharSet = CharSet.Unicode)]
    static extern IntPtr LoadLibraryW(string path);

    [DllImport("kernel32", SetLastError = true, CharSet = CharSet.Unicode)]
    static extern bool SetDllDirectoryW(string path);

    [DllImport("objc.dll", CallingConvention = CallingConvention.Cdecl, CharSet = CharSet.Ansi)]
    static extern IntPtr objc_getClass(string name);

    [DllImport("objc.dll", CallingConvention = CallingConvention.Cdecl, CharSet = CharSet.Ansi)]
    static extern IntPtr sel_registerName(string name);

    [DllImport("objc.dll", EntryPoint = "objc_msgSend", CallingConvention = CallingConvention.Cdecl)]
    static extern IntPtr Send(IntPtr receiver, IntPtr selector);

    [DllImport("objc.dll", EntryPoint = "objc_msgSend", CallingConvention = CallingConvention.Cdecl)]
    static extern IntPtr Send(IntPtr receiver, IntPtr selector, IntPtr arg);

    [DllImport("objc.dll", EntryPoint = "objc_msgSend", CallingConvention = CallingConvention.Cdecl)]
    static extern IntPtr Send(IntPtr receiver, IntPtr selector, byte[] arg);

    static IntPtr NSString(string s)
    {
        byte[] utf8 = Encoding.UTF8.GetBytes(s + "\0");
        return Send(objc_getClass("NSString"), sel_registerName("stringWithUTF8String:"), utf8);
    }

    static string Str(IntPtr obj)
    {
        if (obj == IntPtr.Zero) return "";
        IntPtr p = Send(obj, sel_registerName("UTF8String"));
        return p == IntPtr.Zero ? "" : Marshal.PtrToStringAnsi(p);
    }

    static string Json(string s)
    {
        var b = new StringBuilder();
        foreach (char c in s ?? "")
        {
            if (c == '"' || c == '\\') { b.Append('\\').Append(c); }
            else if (c < ' ') { b.AppendFormat("\\u{0:x4}", (int)c); }
            else b.Append(c);
        }
        return b.ToString();
    }

    static int Fail(string msg)
    {
        Console.WriteLine("{\"error\":\"" + Json(msg) + "\"}");
        return 1;
    }

    static int Main()
    {
        try
        {
            if (IntPtr.Size != 4) return Fail("helper must run as 32-bit");
            // In a 32-bit process this is "C:\Program Files (x86)\Common Files".
            string common = Environment.GetFolderPath(Environment.SpecialFolder.CommonProgramFilesX86);
            if (string.IsNullOrEmpty(common))
                common = Environment.GetFolderPath(Environment.SpecialFolder.CommonProgramFiles);
            string support = Path.Combine(common, @"Apple\Apple Application Support");
            string services = Path.Combine(common, @"Apple\Internet Services");
            string aoskit = Path.Combine(services, "AOSKit.dll");
            if (!File.Exists(Path.Combine(support, "objc.dll")))
                return Fail("Apple Application Support (32-bit) not found. Install the web version of iCloud for Windows.");
            if (!File.Exists(aoskit))
                return Fail("AOSKit.dll not found. Install the web version of iCloud for Windows (not the Microsoft Store one).");

            Environment.SetEnvironmentVariable("PATH", support + ";" + services + ";" + Environment.GetEnvironmentVariable("PATH"));
            SetDllDirectoryW(support);
            Directory.SetCurrentDirectory(support);

            foreach (string dll in new[] {
                Path.Combine(support, "objc.dll"),
                Path.Combine(support, "CoreFoundation.dll"),
                Path.Combine(support, "Foundation.dll"),
                aoskit })
            {
                if (LoadLibraryW(dll) == IntPtr.Zero)
                    return Fail("Couldn't load " + Path.GetFileName(dll) + " (Windows error " + Marshal.GetLastWin32Error() + ")");
            }

            IntPtr utils = objc_getClass("AOSUtilities");
            if (utils == IntPtr.Zero) return Fail("AOSKit doesn't have AOSUtilities (unexpected iCloud version)");

            IntPtr headers = Send(utils, sel_registerName("retrieveOTPHeadersForDSID:"), NSString("-2"));
            if (headers == IntPtr.Zero)
                return Fail("AOSKit returned no sign-in data. Open iCloud for Windows once, then try again.");

            IntPtr getObj = sel_registerName("objectForKey:");
            string md = Str(Send(headers, getObj, NSString("X-Apple-MD")));
            string mdm = Str(Send(headers, getObj, NSString("X-Apple-MD-M")));
            if (md.Length == 0 || mdm.Length == 0)
                return Fail("AOSKit's sign-in data was incomplete.");

            // how iCloud for Windows describes itself (for the log)
            string client = "";
            try { client = Str(Send(Send(objc_getClass("AOSRequest"), sel_registerName("clientInfo")), sel_registerName("description"))); } catch { }

            // This Windows install's own stable machine id, so the device the
            // sign-in data claims to be is the same one every time.
            // This is a 32-bit process, so read the 64-bit registry view
            // explicitly — MachineGuid lives there, not in the WOW6432 view.
            string machineGuid = "";
            try
            {
                using (var hive = Microsoft.Win32.RegistryKey.OpenBaseKey(
                           Microsoft.Win32.RegistryHive.LocalMachine, Microsoft.Win32.RegistryView.Registry64))
                using (var k = hive.OpenSubKey(@"SOFTWARE\Microsoft\Cryptography"))
                {
                    if (k != null) machineGuid = (k.GetValue("MachineGuid") as string ?? "").Trim();
                }
            }
            catch { }

            Console.WriteLine("{\"md\":\"" + Json(md) + "\",\"mdm\":\"" + Json(mdm) + "\",\"icloud\":\"" + Json(client) + "\",\"deviceid\":\"" + Json(machineGuid) + "\"}");
            return 0;
        }
        catch (Exception e)
        {
            return Fail(e.GetType().Name + ": " + e.Message);
        }
    }
}
