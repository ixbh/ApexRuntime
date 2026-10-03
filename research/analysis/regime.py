import sys
exec(open(r'C:\Users\Ivanh\AppData\Local\Temp\claude\C--Users-Ivanh-OneDrive-Desktop-Apex-movement-mod\a2bdc75e-423c-4f0a-8ec8-a40cf16077f9\scratchpad\seg.py').read().split("print('markers'")[0])
starts=[0]+[b+1 for a,b in markers]; ends=[a for a,b in markers]+[len(rows)-1]
def h(r,pre): return (float(r['v_x'])**2+float(r['v_y'])**2)**.5 if pre else r['speed_h']
def show(n, limit=400):
    S=rows[starts[n]:ends[n]]
    k0=next((k for k in range(len(S)) if S[k]['speed_h']>0 or S[k]['b']), 0)
    last=None; out=0
    for k in range(max(k0-1,1),len(S)):
        r=S[k]; d=round(r['speed_h']-h(r,True),1)
        state=(r['ground'],r['duck'],r['slide'],r['sprint'],r['b'],r['fwd'],r['side_move'])
        reg=(state, round(d) if abs(d)>3 else round(d,1))
        if reg!=last:
            print(f"{r['cmd']} ge={r['ground']} duck={r['duck']} sl={r['slide']} spr={r['sprint']} b={r['buttons'][-3:]} fwd={r['fwd']:+.0f} sd={r['side_move']:+.0f} ms={r['max_speed']:6.1f} |h| {h(r,True):7.2f}->{r['speed_h']:7.2f} d={d:+7.2f} vz {r['v_z']:7.1f}->{r['pv_z']:7.1f}")
            out+=1
            if out>limit: break
        last=reg
for n in map(int,sys.argv[1:]): print('== seg',n); show(n,90)
