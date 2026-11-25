# Self-contained single-file Windows release candidate. Reuses existing application code.
from pathlib import Path
import os
import sys
from PyInstaller.utils.hooks import collect_all

root=Path(SPECPATH)
public_release=os.environ.get('QUICKCLIP321_PUBLIC_BUILD') == '1'
python_root=Path(sys.base_prefix)
stdlib=python_root/'Lib'
tkinter_root=stdlib/'tkinter'
tcl_root=python_root/'tcl'
if not tkinter_root.is_dir() or not tcl_root.is_dir():
    raise FileNotFoundError('Self-contained build requires a Python installation with Tcl/Tk')
datas,binaries,hiddenimports=[],[],[]
datas.extend(((str(tkinter_root), 'tkinter'),
              (str(tcl_root/'tcl8.6'), '_tcl_data'),
              (str(tcl_root/'tk8.6'), '_tk_data')))
for name in ('_tkinter.pyd', 'tcl86t.dll', 'tk86t.dll'):
    binary=python_root/'DLLs'/name
    if not binary.is_file():
        raise FileNotFoundError(f'Self-contained build requires {binary}')
    binaries.append((str(binary), '.'))
for package in ('customtkinter','yt_dlp','yt_dlp_ejs'):
    package_data,package_binaries,package_hidden=collect_all(package)
    datas+=package_data
    binaries+=package_binaries
    hiddenimports+=package_hidden
for name in (('node.exe',) if public_release else ('ffmpeg.exe','ffprobe.exe','node.exe')):
    binary=root/'bin'/name
    if not binary.is_file():
        raise FileNotFoundError(f'Self-contained build requires {binary}')
    binaries.append((str(binary),'bin'))
if public_release:
    datas.extend([
        (str(root/'resources'/'bundled-notices'/'NODE-LICENSE.txt'), 'THIRD_PARTY_LICENSES'),
        (str(root/'docs'/'THIRD-PARTY.md'), 'THIRD_PARTY_LICENSES'),
        (str(python_root/'LICENSE.txt'), 'THIRD_PARTY_LICENSES/python-runtime'),
        (str(root/'resources'/'release-notices'), 'THIRD_PARTY_LICENSES/runtime'),
    ])
    for notice_dir in (root/'resources'/'python-notices').iterdir():
        if notice_dir.is_dir() and notice_dir.name != 'mutagen':
            datas.append((str(notice_dir), f'THIRD_PARTY_LICENSES/python/{notice_dir.name}'))
else:
    datas.append((str(root/'resources'/'bundled-notices'),'THIRD_PARTY_LICENSES'))
    datas.append((str(root/'resources'/'python-notices'),'THIRD_PARTY_LICENSES/python'))
datas.append((str(root/'resources'/'QuickClip321Logo.png'),'resources'))
datas.append((str(root/'LICENSE'),'LICENSES'))
a=Analysis([str(root/'test_launcher.py')],pathex=[str(root),str(stdlib)],binaries=binaries,datas=datas,
           hiddenimports=hiddenimports+['tkinter','_tkinter','PIL._tkinter_finder','plyer.platforms.win.notification'],
           hookspath=[str(root/'build_hooks')],hooksconfig={},runtime_hooks=[],
           excludes=['mutagen'] if public_release else [],noarchive=False)
pyz=PYZ(a.pure)
exe=EXE(pyz,a.scripts,a.binaries,a.datas,[],
        name='QuickClip321-2.1.0-rc.1-win-x64' if public_release else 'QuickClip321-2.1.0-rc.1',debug=False,
        bootloader_ignore_signals=False,strip=False,upx=False,console=False,
        icon=str(root/'resources'/'quickclip321.ico'),disable_windowed_traceback=False)
