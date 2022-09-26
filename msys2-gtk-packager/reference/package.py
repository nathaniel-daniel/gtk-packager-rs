import subprocess
import shutil
import os
import sys

BLACKLIST_DLLS = {
    'advapi32.dll',
    
    'cfgmgr32.dll',
    'comctl32.dll',
    'comdlg32.dll',
    'combase.dll',
    'cryptbase.dll',
    'crypt32.dll',
    
    'bcrypt.dll',
    
    'dpapi.dll',
    'dnsapi.dll',
    'dwmapi.dll',
    
    'gdiplus.dll',
    'gdi32full.dll',
    'gdi32.dll',
    
    'hid.dll',
    
    'imm32.dll',
    'iphlpapi.dll',
    
    'msvcrt.dll',
    
    'kernel32.dll',
    'kernelbase.dll',
    
    'msimg32.dll',
    'msvcp_win.dll',
    
    'ntdll.dll',
    
    'ole32.dll',
    
    'rpcrt4.dll',
    
    'shcore.dll',
    'setupapi.dll',
    'sechost.dll',
    'shlwapi.dll',
    'shell32.dll',
    
    'usp10.dll',
    'userenv.dll',
    'user32.dll',
    'ucrtbase.dll',
    
    'winmm.dll',
    'winspool.drv',
    'win32u.dll',
    'ws2_32.dll',
}

def abs_cygpath(path):
    output = subprocess.run(f'cygpath {path} -wa', check=True, capture_output=True, encoding='utf-8')
    return os.path.abspath(output.stdout.strip())

def main():
    target = 'x86_64-pc-windows-gnu'
    profile = 'debug'
    bin_name = 'discord-video-compressor'
    bin_dir = os.path.abspath(f'target/{target}/{profile}')
    
    output = subprocess.run(f'ldd {bin_dir}/{bin_name}.exe', check=True, capture_output=True, encoding='utf-8')
    
    dlls_to_copy = []
    for line in map(str.strip, output.stdout.strip().split('\n')):
        arrow_split = list(map(str.strip, line.split('=>', 1)))
        
        dll_name = arrow_split[0]
        
        if dll_name.lower() in BLACKLIST_DLLS:
            continue
            
        split = arrow_split[1].rsplit(' ', 1)
        dll_path = split[0]
        
        if dll_path.lower().startswith('/c/windows'):
            print(f'Likely invalid dll copy, looks like a system dll: {dll_name} : {dll_path}, skipping...')
            continue
            
        if os.path.exists(f'{bin_dir}/{dll_name}'):
            continue
            
        dll_path = abs_cygpath(dll_path)
        
        if dll_path.lower().startswith(bin_dir.lower()):
            continue
        
        dlls_to_copy.append((dll_name, dll_path))
        
    if not os.path.exists(f'{bin_dir}/{dll_name}'):
        dlls_to_copy.append(('gdbus.exe', abs_cygpath(shutil.which('gdbus'))))
        
    for (dll_name, dll_path) in dlls_to_copy:
        src = dll_path
        dest = f'{bin_dir}/{dll_name}'
        print(f'{src} => {dest}')
        shutil.copyfile(src, dest)
    
if __name__ == "__main__":
    main()