"""Windows AppContainer, zero capabilities, verified before the child resumes."""
import ctypes as C
import ctypes.wintypes as W
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import time
from contextlib import contextmanager
from .storage import clean_tree

P = C.c_void_p
SZ = C.c_size_t

class SI(C.Structure):
    _fields_ = [('cb',W.DWORD),('lpReserved',W.LPWSTR),('lpDesktop',W.LPWSTR),('lpTitle',W.LPWSTR),('dwX',W.DWORD),('dwY',W.DWORD),('dwXSize',W.DWORD),('dwYSize',W.DWORD),('dwXCountChars',W.DWORD),('dwYCountChars',W.DWORD),('dwFillAttribute',W.DWORD),('dwFlags',W.DWORD),('wShowWindow',W.WORD),('cbReserved2',W.WORD),('lpReserved2',P),('hStdInput',W.HANDLE),('hStdOutput',W.HANDLE),('hStdError',W.HANDLE)]
class SIX(C.Structure):
    _fields_ = [('si',SI),('attrs',P)]
class PI(C.Structure):
    _fields_ = [('process',W.HANDLE),('thread',W.HANDLE),('pid',W.DWORD),('tid',W.DWORD)]
class CAPS(C.Structure):
    _fields_ = [('sid',P),('capabilities',P),('count',W.DWORD),('reserved',W.DWORD)]
class BASIC(C.Structure):
    _fields_ = [('perProcess',C.c_int64),('perJob',C.c_int64),('limitFlags',W.DWORD),('minWorking',SZ),('maxWorking',SZ),('activeProcesses',W.DWORD),('affinity',SZ),('priority',W.DWORD),('scheduling',W.DWORD)]
class IO(C.Structure):
    _fields_ = [(n,C.c_uint64) for n in ('rOp','wOp','oOp','rBytes','wBytes','oBytes')]
class LIMITS(C.Structure):
    _fields_ = [('basic',BASIC),('io',IO),('procMem',SZ),('jobMem',SZ),('peakProc',SZ),('peakJob',SZ)]

class Container:
    def __init__(self, store):
        if os.name != 'nt':
            raise RuntimeError('Windows 10/11 x64 wird benötigt')
        self.store = store
        self.name = 'HIPreservation.Tester.' + hashlib.sha256(str(store.root).encode()).hexdigest()[:20]
        self.k = k = C.WinDLL('kernel32',use_last_error=True)
        self.a = a = C.WinDLL('advapi32',use_last_error=True)
        self.u = u = C.WinDLL('userenv',use_last_error=True)
        def api(lib,name,args,result):
            fn = getattr(lib,name);fn.argtypes=args;fn.restype=result
        api(u,'CreateAppContainerProfile',[W.LPCWSTR,W.LPCWSTR,W.LPCWSTR,P,W.DWORD,C.POINTER(P)],C.c_long)
        api(u,'DeriveAppContainerSidFromAppContainerName',[W.LPCWSTR,C.POINTER(P)],C.c_long)
        api(a,'ConvertSidToStringSidW',[P,C.POINTER(W.LPWSTR)],W.BOOL)
        api(a,'OpenProcessToken',[W.HANDLE,W.DWORD,C.POINTER(W.HANDLE)],W.BOOL)
        api(a,'GetTokenInformation',[W.HANDLE,C.c_int,P,W.DWORD,C.POINTER(W.DWORD)],W.BOOL)
        api(k,'InitializeProcThreadAttributeList',[P,W.DWORD,W.DWORD,C.POINTER(SZ)],W.BOOL)
        api(k,'UpdateProcThreadAttribute',[P,W.DWORD,SZ,P,SZ,P,P],W.BOOL)
        api(k,'CreateProcessW',[W.LPCWSTR,W.LPWSTR,P,P,W.BOOL,W.DWORD,P,W.LPCWSTR,C.POINTER(SIX),C.POINTER(PI)],W.BOOL)
        api(k,'SetDllDirectoryW',[W.LPCWSTR],W.BOOL)
        api(k,'CreateJobObjectW',[P,W.LPCWSTR],W.HANDLE)
        api(k,'SetInformationJobObject',[W.HANDLE,C.c_int,P,W.DWORD],W.BOOL)
        api(k,'AssignProcessToJobObject',[W.HANDLE,W.HANDLE],W.BOOL)
        api(k,'ResumeThread',[W.HANDLE],W.DWORD)
        api(k,'WaitForSingleObject',[W.HANDLE,W.DWORD],W.DWORD)
        api(k,'TerminateProcess',[W.HANDLE,W.UINT],W.BOOL)
        api(k,'GetExitCodeProcess',[W.HANDLE,C.POINTER(W.DWORD)],W.BOOL)
        api(k,'CloseHandle',[W.HANDLE],W.BOOL)
        api(k,'DeleteProcThreadAttributeList',[P],None)
        api(k,'LocalFree',[P],P)
        api(a,'FreeSid',[P],P)
        self.sid = P()
        hr = u.CreateAppContainerProfile(self.name,'Hunter Island tester','Local offline game profile',None,0,C.byref(self.sid))
        if hr < 0:
            if hr & 0xffffffff != 0x800700b7:
                raise OSError('AppContainer konnte nicht erstellt werden: '+hex(hr & 0xffffffff))
            hr = u.DeriveAppContainerSidFromAppContainerName(self.name,C.byref(self.sid))
            if hr < 0:
                raise OSError('AppContainer-SID konnte nicht bestimmt werden')
        self.sid_text = self.text_sid(self.sid)

    def close(self):
        if self.sid:
            self.a.FreeSid(self.sid)
            self.sid = P()

    @staticmethod
    def check(value):
        if not value:
            raise C.WinError(C.get_last_error())
        return value

    def text_sid(self,sid):
        st = W.LPWSTR()
        self.check(self.a.ConvertSidToStringSidW(sid,C.byref(st)))
        value = st.value
        self.k.LocalFree(C.cast(st,P))
        return value

    @contextmanager
    def system_dll_search(self):
        # The frozen launcher needs its bundled DLLs. Its child programs use
        # the Windows search order and their own runtime folder.
        launcher_dlls = getattr(sys, '_MEIPASS', None)
        if launcher_dlls is not None:
            self.check(self.k.SetDllDirectoryW(None))
        try:
            yield
        finally:
            if launcher_dlls is not None:
                self.check(self.k.SetDllDirectoryW(launcher_dlls))

    def prepare(self):
        clean_tree(self.store.run)
        target = self.store.run.resolve()
        if target != self.store.root / 'profiles/default/run':
            raise ValueError('Unerwartetes Isolationsverzeichnis')
        icacls = Path(os.environ['SystemRoot']) / 'System32/icacls.exe'
        for flags in (['/grant',f'*{self.sid_text}:(OI)(CI)(M)','/T','/Q'],['/setintegritylevel','(OI)(CI)L','/T','/Q']):
            with self.system_dll_search():
                subprocess.run([str(icacls),str(target),*flags],check=True,capture_output=True,creationflags=0x08000000)

    def token_info(self,token,kind):
        size = W.DWORD()
        self.a.GetTokenInformation(token,kind,None,0,C.byref(size))
        buf = C.create_string_buffer(size.value)
        self.check(self.a.GetTokenInformation(token,kind,buf,size,C.byref(size)))
        return buf

    def launch(self, executable, args, label='game', timeout=None, visible=True):
        import msvcrt
        executable = Path(executable).resolve()
        if executable.parent != self.store.run or executable.name not in {'touchHLE.exe','isolation-probe.exe'}:
            raise ValueError('Nur Programme aus dem geprüften Profil dürfen starten')
        self.store.logs.mkdir(exist_ok=True)
        log = self.store.logs / (label+'.log')
        fd = os.open(log,os.O_CREAT|os.O_TRUNC|os.O_WRONLY|os.O_BINARY,0o600)
        inn = os.open('NUL',os.O_RDONLY)
        os.set_inheritable(fd,True);os.set_inheritable(inn,True)
        k = self.k
        size = SZ()
        k.InitializeProcThreadAttributeList(None,2,0,C.byref(size))
        attrs = C.create_string_buffer(size.value)
        self.check(k.InitializeProcThreadAttributeList(attrs,2,0,C.byref(size)))
        pi = PI();token = W.HANDLE();job = None
        result = dict(is_appcontainer=False,capabilities=None,resumed=False)
        try:
            caps = CAPS(self.sid,None,0,0)
            self.check(k.UpdateProcThreadAttribute(attrs,0,0x20009,C.byref(caps),C.sizeof(caps),None,None))
            handles = (W.HANDLE*2)(msvcrt.get_osfhandle(fd),msvcrt.get_osfhandle(inn))
            self.check(k.UpdateProcThreadAttribute(attrs,0,0x20002,handles,C.sizeof(handles),None,None))
            si = SIX();si.si.cb=C.sizeof(si);si.si.dwFlags=0x101;si.si.wShowWindow=1 if visible else 0
            si.si.hStdInput=handles[1];si.si.hStdOutput=handles[0];si.si.hStdError=handles[0];si.attrs=C.cast(attrs,P)
            windows = os.environ['SystemRoot']
            env = dict(SystemRoot=windows,WINDIR=windows,PATH=str(Path(windows)/'System32'),
                TEMP=str(self.store.run/'temp'),TMP=str(self.store.run/'temp'),
                USERPROFILE=str(self.store.run/'profile'),LOCALAPPDATA=str(self.store.run/'profile'),
                APPDATA=str(self.store.run/'profile'),RUST_BACKTRACE='1')
            block = C.create_unicode_buffer('\0'.join(f'{key}={value}' for key,value in sorted(env.items()))+'\0\0')
            with self.system_dll_search():
                self.check(k.CreateProcessW(str(executable),C.create_unicode_buffer(subprocess.list2cmdline([str(executable),*args])),None,None,True,
                    0x80000|4|0x400|0x08000000,block,str(self.store.run),C.byref(si),C.byref(pi)))
            self.check(self.a.OpenProcessToken(pi.process,8,C.byref(token)))
            isac = C.cast(self.token_info(token,29),C.POINTER(W.DWORD))[0]
            capabilities = C.cast(self.token_info(token,30),C.POINTER(W.DWORD))[0]
            sidbuf = self.token_info(token,31)
            actual_sid = self.text_sid(C.cast(sidbuf,C.POINTER(P))[0])
            if isac != 1 or capabilities != 0 or actual_sid != self.sid_text:
                raise RuntimeError('Isolationsprüfung fehlgeschlagen; kein ungeschützter Ersatzstart')
            result.update(is_appcontainer=True,capabilities=0,sid_verified=True)
            job = self.check(k.CreateJobObjectW(None,None))
            limits = LIMITS();limits.basic.limitFlags=0x2000
            self.check(k.SetInformationJobObject(job,9,C.byref(limits),C.sizeof(limits)))
            self.check(k.AssignProcessToJobObject(job,pi.process))
            if k.ResumeThread(pi.thread) == 0xffffffff:
                raise C.WinError(C.get_last_error())
            result['resumed']=True
            waited = k.WaitForSingleObject(pi.process,0xffffffff if timeout is None else int(timeout*1000))
            if waited == 258:
                self.check(k.TerminateProcess(pi.process,124));k.WaitForSingleObject(pi.process,5000)
            elif waited != 0:
                raise RuntimeError('Warten auf den Spielprozess fehlgeschlagen')
            code = W.DWORD();self.check(k.GetExitCodeProcess(pi.process,C.byref(code)))
            result.update(exit_code=code.value,timed_out=waited==258)
            return result
        except BaseException:
            if pi.process:
                k.TerminateProcess(pi.process,125)
            raise
        finally:
            for handle in (token,pi.thread,pi.process,job):
                if handle:
                    k.CloseHandle(handle)
            k.DeleteProcThreadAttributeList(attrs)
            os.close(fd);os.close(inn)
            (self.store.logs/(label+'-isolation.json')).write_text(json.dumps(result,indent=2))

    def probe(self):
        canary = self.store.root / 'private-isolation-canary.txt'
        canary.write_text('Container must not be able to read this outside its run directory.')
        try:
            result = self.launch(self.store.run/'isolation-probe.exe',[str(canary)],'probe',timeout=20,visible=False)
            if result['exit_code'] != 0 or 'ISOLATION_PROBE_PASS' not in (self.store.logs/'probe.log').read_text():
                raise RuntimeError('Isolationsprobe fehlgeschlagen; Spiel wird nicht gestartet')
        finally:
            canary.unlink(missing_ok=True)
