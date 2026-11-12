#!/bin/bash
# run.sh <name> <e51|e52> [overlay]
S=/tmp/claude-1000/-home-emir-Dev-rein-app/dc0728de-90eb-47c2-a284-9fb1ce48655a/scratchpad/conpty; W=$(wslpath -w $S)
if [ "$2" = e51 ]; then sc="PATH=$S/bin:\$PATH; tty; exec ~/.rein/bin/rein start"; else sc="PATH=$S/bin:\$PATH; exec ~/.rein/bin/rein start"; fi
side=""; [ "$3" = overlay ] && side="-d Ubuntu -e $S/ovwrap.sh {TTY} hidden #1e1e2e #cdd6f4 #a6adc8 #45475a #89b4fa"
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "$W\\ConPtyProbe.ps1" -Cmd "C:\\Windows\\System32\\wsl.exe -d Ubuntu --cd $S/proj -e /bin/zsh -lic \"$sc\"" -Out "$W\\$1" -Seconds ${SECS:-8} -Side "$side" -SideIn "$W\\overlay.sh"
python3 - "$S/$1" <<'PY'
import sys,re
b=open(sys.argv[1]+'.bin','rb').read(); log=open(sys.argv[1]+'.log').read().splitlines()
off=b.find(b'\x1b[?1049h'); t=None
for l in log:
    m=re.match(r'\s*(\d+) ms  read\s+(\d+) bytes at (\d+)',l)
    if m and off>=0 and int(m[3])<=off<int(m[3])+int(m[2]): t=int(m[1])
print(sys.argv[1].split('/')[-1], 'first tmux frame (1049h) at', t, 'ms;', [l.strip() for l in log if 'overlay' in l or 'tty' in l])
PY
