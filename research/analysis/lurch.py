"""Per airborne arc: every input change in the air with time since takeoff,
heading change and speed change on that tick. Usage: lurch.py <csv> <seg...>"""
import sys, math
sys.argv=[sys.argv[0], sys.argv[1]]+sys.argv[2:]
csv_path=sys.argv[1]; segs=[int(s) for s in sys.argv[2:]]
_src=open(__file__.replace('lurch.py','seg.py'),encoding='utf-8').read().split("print('markers'")[0]
_old=sys.argv; sys.argv=[_old[0], csv_path]; exec(_src); sys.argv=_old
starts=[0]+[b+1 for a,b in markers]; ends=[a for a,b in markers]+[len(rows)-1]
hd=lambda x,y: math.degrees(math.atan2(y,x))
H=lambda r,pre: (float(r['v_x'])**2+float(r['v_y'])**2)**.5 if pre else r['speed_h']
for n in segs:
    S=rows[starts[n]:ends[n]]; print(f"===== seg {n}")
    arc=None
    for k in range(1,len(S)):
        r=S[k]; p=S[k-1]
        if r['ground']==0 and p['ground']==1:
            arc=dict(t0=r['game_time'],h0=hd(float(r['pv_x']),float(r['pv_y'])),s0=r['speed_h'],ev=[])
        if arc and r['ground']==0:
            h_pre=hd(float(r['v_x']),float(r['v_y'])); h_post=hd(float(r['pv_x']),float(r['pv_y']))
            dh=(h_post-h_pre+180)%360-180; ds=H(r,0)-H(r,1)
            if r['b']!=p['b'] or abs(dh)>0.5 or abs(ds)>0.5:
                arc['ev'].append(f"{r['game_time']-arc['t0']:6.3f}s b={r['buttons'][-4:]} hd {h_pre:7.2f}->{h_post:7.2f} (d{dh:+6.2f}) |h| {H(r,1):6.1f}->{H(r,0):6.1f} (d{ds:+6.2f})")
        if arc and r['ground']==1 and p['ground']==0:
            print(f"arc air {p['game_time']-arc['t0']:.3f}s start |h| {arc['s0']:.1f}")
            for e in arc['ev'][:14]: print("    ",e)
            arc=None
