import sys, struct, array
import numpy as np
def read(p):
    b = open(p, 'rb').read(); i = 12
    while i < len(b):
        cid, size = b[i:i+4], struct.unpack('<I', b[i+4:i+8])[0]
        if cid == b'data':
            a = np.frombuffer(b[i+8:i+8+size], dtype='<f4')
            return a.reshape(-1, 2).mean(axis=1)
        i += 8 + size + (size & 1)
sr = 48000
rows = {}
for p in sys.argv[1:]:
    x = read(p)
    out = []
    for s in range(8, 16):
        seg = x[s*sr//2*2: s*sr//2*2 + sr//2] if False else x[int(s*0.5*sr):int((s+1)*0.5*sr)]
        rms = float(np.sqrt(np.mean(seg**2)))
        spec = np.abs(np.fft.rfft(seg * np.hanning(len(seg))))
        f = np.fft.rfftfreq(len(seg), 1/sr)
        cent = float((spec*f).sum()/max(spec.sum(), 1e-12))
        out.append((rms, cent))
    rows[p] = out
names = list(rows)
print('window  ' + '  '.join('%-18s' % n.split('/')[-1] for n in names))
for s in range(8, 16):
    print('%.1f-%.1fs ' % (s*0.5, (s+1)*0.5) + '  '.join('rms %.4f c %5.0f   ' % rows[n][s-8] for n in names))
