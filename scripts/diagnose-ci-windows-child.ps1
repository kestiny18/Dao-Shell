param([string]$TestHarness = '')
# Diagnostic only: no accounts, policies, object ACLs or global station changes.
$ErrorActionPreference = 'Stop'
if ($TestHarness) {
    $probeHarness = [IO.Path]::GetFullPath($TestHarness)
    $probeRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
    $probeDeps = [IO.Path]::Combine($probeRoot, 'target', 'debug', 'deps') + [IO.Path]::DirectorySeparatorChar
    if (-not $probeHarness.StartsWith($probeDeps, [StringComparison]::OrdinalIgnoreCase) -or
        [IO.Path]::GetFileName($probeHarness) -notmatch '^dao_shell-[a-fA-F0-9]+\.exe$' -or
        -not [IO.File]::Exists($probeHarness)) { throw 'Expected a built core test harness under target/debug/deps' }
} else { $probeHarness = '' }
Add-Type -TypeDefinition @'
using System;
using System.IO;
using System.Text;
using System.Collections.Generic;
using System.ComponentModel;
using System.Runtime.InteropServices;
public static class DaoChildProbe {
    [StructLayout(LayoutKind.Sequential)] struct SidEntry { public IntPtr Sid; public uint Attributes; }
    [StructLayout(LayoutKind.Sequential)] struct Startup {
        public uint cb; public IntPtr reserved, desktop, title;
        public uint x,y,xs,ys,xc,yc,fill,flags; public ushort show, reservedSize;
        public IntPtr reservedBytes, input, output, error;
    }
    [StructLayout(LayoutKind.Sequential)] struct StartupEx { public Startup startup; public IntPtr attributes; }
    [StructLayout(LayoutKind.Sequential)] struct ProcessInfo { public IntPtr process, thread; public uint pid, tid; }
    [DllImport("kernel32.dll")] static extern IntPtr GetCurrentProcess();
    [DllImport("kernel32.dll")] static extern uint GetCurrentThreadId();
    [DllImport("kernel32.dll")] static extern bool CloseHandle(IntPtr h);
    [DllImport("kernel32.dll")] static extern IntPtr LocalFree(IntPtr h);
    [DllImport("kernel32.dll",SetLastError=true)] static extern bool SetHandleInformation(IntPtr h,uint mask,uint flags);
    [DllImport("kernel32.dll",CharSet=CharSet.Unicode)] static extern uint GetSystemDirectoryW(StringBuilder s,uint n);
    [DllImport("advapi32.dll",SetLastError=true)] static extern bool OpenProcessToken(IntPtr p,uint access,out IntPtr token);
    [DllImport("advapi32.dll",SetLastError=true)] static extern bool SetTokenInformation(IntPtr t,int kind,IntPtr data,uint n);
    [DllImport("advapi32.dll")] static extern uint GetLengthSid(IntPtr sid);
    [DllImport("advapi32.dll",SetLastError=true)] static extern bool GetTokenInformation(IntPtr t,int kind,IntPtr data,uint n,out uint returned);
    [DllImport("advapi32.dll",SetLastError=true)] static extern bool CreateRestrictedToken(IntPtr t,uint flags,uint disables,IntPtr sids,uint deletes,IntPtr privileges,uint restricts,IntPtr restricted,out IntPtr result);
    [DllImport("advapi32.dll",CharSet=CharSet.Unicode,SetLastError=true)] static extern bool ConvertStringSidToSidW(string text,out IntPtr sid);
    [DllImport("advapi32.dll")] static extern bool IsWellKnownSid(IntPtr sid,int kind);
    [DllImport("advapi32.dll")] static extern IntPtr GetSidSubAuthorityCount(IntPtr sid);
    [DllImport("advapi32.dll")] static extern IntPtr GetSidSubAuthority(IntPtr sid,uint index);
    [DllImport("advapi32.dll",SetLastError=true)] static extern bool SaferCreateLevel(uint scope,uint level,uint flags,out IntPtr result,IntPtr reserved);
    [DllImport("advapi32.dll",SetLastError=true)] static extern bool SaferComputeTokenFromLevel(IntPtr level,IntPtr token,out IntPtr result,uint flags,IntPtr reserved);
    [DllImport("advapi32.dll")] static extern bool SaferCloseLevel(IntPtr level);
    [DllImport("advapi32.dll",SetLastError=true)] static extern bool ImpersonateLoggedOnUser(IntPtr t);
    [DllImport("advapi32.dll",SetLastError=true)] static extern bool RevertToSelf();
    [DllImport("user32.dll")] static extern IntPtr GetProcessWindowStation();
    [DllImport("user32.dll")] static extern IntPtr GetThreadDesktop(uint thread);
    [DllImport("user32.dll",CharSet=CharSet.Unicode,SetLastError=true)] static extern bool GetUserObjectInformationW(IntPtr h,int kind,StringBuilder s,uint n,out uint needed);
    [DllImport("user32.dll",CharSet=CharSet.Unicode,SetLastError=true)] static extern IntPtr OpenWindowStationW(string name,bool inherit,uint access);
    [DllImport("user32.dll",CharSet=CharSet.Unicode,SetLastError=true)] static extern IntPtr OpenDesktopW(string name,uint flags,bool inherit,uint access);
    [DllImport("user32.dll")] static extern bool CloseWindowStation(IntPtr h);
    [DllImport("user32.dll")] static extern bool CloseDesktop(IntPtr h);
    [DllImport("kernel32.dll",SetLastError=true)] static extern bool InitializeProcThreadAttributeList(IntPtr p,uint count,uint flags,ref UIntPtr size);
    [DllImport("kernel32.dll",SetLastError=true)] static extern bool UpdateProcThreadAttribute(IntPtr list,uint flags,UIntPtr attr,IntPtr value,UIntPtr size,IntPtr previous,IntPtr returned);
    [DllImport("kernel32.dll")] static extern void DeleteProcThreadAttributeList(IntPtr p);
    [DllImport("advapi32.dll",CharSet=CharSet.Unicode,SetLastError=true)] static extern bool CreateProcessAsUserW(IntPtr token,string app,StringBuilder args,IntPtr p,IntPtr t,bool inherit,uint flags,IntPtr env,string cwd,ref StartupEx startup,out ProcessInfo info);
    [DllImport("kernel32.dll",SetLastError=true)] static extern uint WaitForSingleObject(IntPtr p,uint timeout);
    [DllImport("kernel32.dll",SetLastError=true)] static extern bool GetExitCodeProcess(IntPtr p,out uint code);
    [DllImport("kernel32.dll",SetLastError=true)] static extern bool TerminateProcess(IntPtr p,uint code);
    public sealed class Result {
        public string TokenKind; public int ApiError;
        public bool? Elevated, AdminEnabled, SameSession, StationRead, DesktopRead, StationAllAccess, DesktopAllAccess;
        public uint? IntegrityRid; public int StationError, DesktopError, StationAllError, DesktopAllError;
        public List<Launch> Children = new List<Launch>();
    }
    public sealed class Launch { public string Image; public int CreateError; public bool TimedOut; public string Exit; public long OutputBytes; }
    static void Check(bool ok) { if(!ok) throw new Win32Exception(Marshal.GetLastWin32Error()); }
    static IntPtr Info(IntPtr t,int kind) {
        uint n; GetTokenInformation(t,kind,IntPtr.Zero,0,out n);
        if(n==0 || n>65536) throw new InvalidOperationException();
        IntPtr p=Marshal.AllocHGlobal((int)n);
        try { Check(GetTokenInformation(t,kind,p,n,out n)); return p; }
        catch { Marshal.FreeHGlobal(p); throw; }
    }
    static int Integer(IntPtr t,int kind) { var p=Info(t,kind); try{return Marshal.ReadInt32(p);}finally{Marshal.FreeHGlobal(p);} }
    static string ObjectName(IntPtr h) {
        var s=new StringBuilder(1024); uint n; Check(GetUserObjectInformationW(h,2,s,2048,out n)); return s.ToString();
    }
    static IntPtr Token(IntPtr original,string kind) {
        IntPtr result;
        if(kind=="Original") return original;
        if(kind=="SaferNormalUser") {
            IntPtr level; Check(SaferCreateLevel(2,0x20000,1,out level,IntPtr.Zero));
            try{Check(SaferComputeTokenFromLevel(level,original,out result,0,IntPtr.Zero));return result;}
            finally{SaferCloseLevel(level);}
        }
        if(kind=="SaferThenLuaDisableMax") {
            var safer=Token(original,"SaferNormalUser");
            try{Check(CreateRestrictedToken(safer,5,0,IntPtr.Zero,0,IntPtr.Zero,0,IntPtr.Zero,out result));return result;}
            finally{CloseHandle(safer);}
        }
        if(kind=="LuaDisableMax" || kind=="LuaOnly" || kind=="DisableMaxOnly" || kind=="LuaDisableMaxMedium") {
            uint flags=kind=="LuaOnly"?4u:kind=="DisableMaxOnly"?1u:5u;
            Check(CreateRestrictedToken(original,flags,0,IntPtr.Zero,0,IntPtr.Zero,0,IntPtr.Zero,out result));
            try{if(kind=="LuaDisableMaxMedium") LowerMedium(result);return result;}
            catch{CloseHandle(result);throw;}
        }
        IntPtr admin; Check(ConvertStringSidToSidW("S-1-5-32-544",out admin));
        IntPtr entry=Marshal.AllocHGlobal(Marshal.SizeOf(typeof(SidEntry)));
        try {
            Marshal.StructureToPtr(new SidEntry{Sid=admin},entry,false);
            Check(CreateRestrictedToken(original,1,1,entry,0,IntPtr.Zero,0,IntPtr.Zero,out result));return result;
        } finally {Marshal.FreeHGlobal(entry);LocalFree(admin);}
    }
    static void LowerMedium(IntPtr token) {
        // Only this newly created token is changed. Never raise integrity or alter the host token.
        var current=Info(token,25);
        try {
            var label=Marshal.ReadIntPtr(current);var n=Marshal.ReadByte(GetSidSubAuthorityCount(label));
            if((uint)Marshal.ReadInt32(GetSidSubAuthority(label,(uint)(n-1)))<8192) throw new InvalidOperationException();
        }finally{Marshal.FreeHGlobal(current);}
        IntPtr sid;Check(ConvertStringSidToSidW("S-1-16-8192",out sid));
        IntPtr entry=Marshal.AllocHGlobal(Marshal.SizeOf(typeof(SidEntry)));
        try {
            Marshal.StructureToPtr(new SidEntry{Sid=sid,Attributes=0x20},entry,false);
            Check(SetTokenInformation(token,25,entry,(uint)Marshal.SizeOf(typeof(SidEntry))+GetLengthSid(sid)));
        }finally{Marshal.FreeHGlobal(entry);LocalFree(sid);}
    }
    static void Describe(IntPtr token,IntPtr original,Result r,string station,string desktop) {
        r.Elevated=Integer(token,20)!=0; r.SameSession=Integer(token,12)==Integer(original,12);
        var integrity=Info(token,25);
        try {var sid=Marshal.ReadIntPtr(integrity);var count=Marshal.ReadByte(GetSidSubAuthorityCount(sid));r.IntegrityRid=(uint)Marshal.ReadInt32(GetSidSubAuthority(sid,(uint)(count-1)));}
        finally{Marshal.FreeHGlobal(integrity);}
        var groups=Info(token,2);
        try {
            r.AdminEnabled=false; int count=Marshal.ReadInt32(groups); int offset=IntPtr.Size;
            for(int i=0;i<count;i++) {
                var group=(SidEntry)Marshal.PtrToStructure(IntPtr.Add(groups,offset+i*Marshal.SizeOf(typeof(SidEntry))),typeof(SidEntry));
                if(IsWellKnownSid(group.Sid,26)) r.AdminEnabled=(group.Attributes&4)!=0;
            }
        }finally{Marshal.FreeHGlobal(groups);}
        Check(ImpersonateLoggedOnUser(token));
        try {
            var ws=OpenWindowStationW(station,false,2);r.StationRead=ws!=IntPtr.Zero;r.StationError=ws==IntPtr.Zero?Marshal.GetLastWin32Error():0;if(ws!=IntPtr.Zero)CloseWindowStation(ws);
            var ds=OpenDesktopW(desktop,0,false,1);r.DesktopRead=ds!=IntPtr.Zero;r.DesktopError=ds==IntPtr.Zero?Marshal.GetLastWin32Error():0;if(ds!=IntPtr.Zero)CloseDesktop(ds);
            // Requesting rights does not modify either object; no write API is used.
            var allWs=OpenWindowStationW(station,false,0xF037F);r.StationAllAccess=allWs!=IntPtr.Zero;r.StationAllError=allWs==IntPtr.Zero?Marshal.GetLastWin32Error():0;if(allWs!=IntPtr.Zero)CloseWindowStation(allWs);
            var allDs=OpenDesktopW(desktop,0,false,0xF01FF);r.DesktopAllAccess=allDs!=IntPtr.Zero;r.DesktopAllError=allDs==IntPtr.Zero?Marshal.GetLastWin32Error():0;if(allDs!=IntPtr.Zero)CloseDesktop(allDs);
        }finally {if(!RevertToSelf()) Environment.FailFast("Probe could not revert thread impersonation");}
    }
    static Launch Spawn(IntPtr token,string image,string arguments,string label) {
        var r=new Launch{Image=label};
        string log=Path.Combine(Path.GetTempPath(),"dao-child-probe-"+Guid.NewGuid().ToString("N")+".log");
        IntPtr attr=IntPtr.Zero,list=IntPtr.Zero,desktop=IntPtr.Zero;
        ProcessInfo info=new ProcessInfo();bool initialized=false,finished=false;
        try {
            using(var output=new FileStream(log,FileMode.CreateNew,FileAccess.Write,FileShare.ReadWrite))
            using(var input=new FileStream("NUL",FileMode.Open,FileAccess.Read,FileShare.ReadWrite)) {
                var oh=output.SafeFileHandle.DangerousGetHandle();var ih=input.SafeFileHandle.DangerousGetHandle();
                Check(SetHandleInformation(oh,1,1));Check(SetHandleInformation(ih,1,1));
                UIntPtr size=UIntPtr.Zero;InitializeProcThreadAttributeList(IntPtr.Zero,1,0,ref size);
                attr=Marshal.AllocHGlobal(checked((int)size.ToUInt64()));Check(InitializeProcThreadAttributeList(attr,1,0,ref size));initialized=true;
                list=Marshal.AllocHGlobal(IntPtr.Size*2);Marshal.WriteIntPtr(list,oh);Marshal.WriteIntPtr(list,IntPtr.Size,ih);
                Check(UpdateProcThreadAttribute(attr,0,(UIntPtr)0x20002,list,(UIntPtr)(IntPtr.Size*2),IntPtr.Zero,IntPtr.Zero));
                desktop=Marshal.StringToHGlobalUni("");
                var startup=new StartupEx{startup=new Startup{cb=(uint)Marshal.SizeOf(typeof(StartupEx)),desktop=desktop,flags=0x100,input=ih,output=oh,error=oh},attributes=attr};
                bool created=CreateProcessAsUserW(token,image,new StringBuilder("\""+image+"\" "+arguments),IntPtr.Zero,IntPtr.Zero,true,0x08080000,IntPtr.Zero,null,ref startup,out info);
                int err=Marshal.GetLastWin32Error();SetHandleInformation(oh,1,0);SetHandleInformation(ih,1,0);
                if(!created){r.CreateError=err;return r;}
                uint wait=WaitForSingleObject(info.process,10000);
                r.TimedOut=wait==258; if(wait!=0){r.CreateError=r.TimedOut?0:Marshal.GetLastWin32Error();return r;}
                finished=true;uint exit;Check(GetExitCodeProcess(info.process,out exit));r.Exit="0x"+exit.ToString("X8");
            }
            r.OutputBytes=new FileInfo(log).Length;return r;
        } catch(Win32Exception e){r.CreateError=e.NativeErrorCode;return r;}
        finally {
            if(info.process!=IntPtr.Zero){if(!finished){TerminateProcess(info.process,1);WaitForSingleObject(info.process,5000);}CloseHandle(info.process);}
            if(info.thread!=IntPtr.Zero)CloseHandle(info.thread);
            if(initialized)DeleteProcThreadAttributeList(attr);
            if(attr!=IntPtr.Zero)Marshal.FreeHGlobal(attr);if(list!=IntPtr.Zero)Marshal.FreeHGlobal(list);if(desktop!=IntPtr.Zero)Marshal.FreeHGlobal(desktop);
            if(File.Exists(log))File.Delete(log);
        }
    }
    public static List<Result> Run(string harness) {
        var result=new List<Result>();IntPtr original;Check(OpenProcessToken(GetCurrentProcess(),0x8B,out original));
        try {
            var system=new StringBuilder(32768);if(GetSystemDirectoryW(system,(uint)system.Capacity)==0)throw new Win32Exception();
            var cmd=Path.Combine(system.ToString(),"cmd.exe");
            var station=ObjectName(GetProcessWindowStation());var desktop=ObjectName(GetThreadDesktop(GetCurrentThreadId()));
            foreach(string kind in new[]{"Original","LuaDisableMax","SaferNormalUser","AdminDenyDisableMax","LuaOnly","DisableMaxOnly","LuaDisableMaxMedium","SaferThenLuaDisableMax"}) {
                var r=new Result{TokenKind=kind};IntPtr token=IntPtr.Zero;
                try {
                    token=Token(original,kind);Describe(token,original,r,station,desktop);
                    r.Children.Add(Spawn(token,cmd,"/d /c exit 0","SystemCmd"));
                    if(!String.IsNullOrEmpty(harness))r.Children.Add(Spawn(token,harness,"--list","RustHarnessList"));
                }catch(Win32Exception e){r.ApiError=e.NativeErrorCode;}
                finally{if(token!=IntPtr.Zero && token!=original)CloseHandle(token);}
                result.Add(r);
            }
        }finally{CloseHandle(original);}
        return result;
    }
}
'@
[DaoChildProbe]::Run($probeHarness) | ConvertTo-Json -Depth 5
if (-not $probeHarness) { Write-Output 'RustHarnessList: NOT_RUN (optional built harness not supplied)' }
