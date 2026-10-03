import csv
rows=list(csv.DictReader(open(__import__('sys').argv[1] if len(__import__('sys').argv)>1 and __import__('sys').argv[1].endswith('.csv') else 'traces/flowstate-session1.csv')))
for r in rows:
    for k in ('duck','ground','slide','sprint'): r[k]=int(r[k])
    for k in ('speed_h','pv_z','v_z','game_time','fwd','side_move','max_speed'): r[k]=float(r[k] or 0)
    r['b']=int(r['buttons'],16)
# duck press edges while nearly stationary
edges=[i for i in range(1,len(rows)) if rows[i]['duck'] and not rows[i-1]['duck'] and rows[i]['speed_h']<30]
# group edges into markers: >=3 edges within 3 s
markers=[]; i=0
while i<len(edges):
    j=i
    while j+1<len(edges) and rows[edges[j+1]]['game_time']-rows[edges[i]]['game_time']<3.0: j+=1
    if j-i+1>=3: markers.append((edges[i],edges[j])); i=j+1
    else: i+=1
print('markers',len(markers),[round(rows[a]['game_time'],1) for a,b in markers])
bounds=[0]+[b for a,b in markers]+[len(rows)-1]
starts=[0]+[b+1 for a,b in markers]
ends=[a for a,b in markers]+[len(rows)-1]
for n,(s,e) in enumerate(zip(starts,ends)):
    seg=rows[s:e]
    if not seg: continue
    jumps=sum(1 for k in range(1,len(seg)) if seg[k]['pv_z']>100 and seg[k-1]['ground']==1 and seg[k]['ground']==0)
    slides=sum(1 for k in range(1,len(seg)) if seg[k]['slide'] and not seg[k-1]['slide'])
    print(f"seg{n:2d} t={seg[0]['game_time']:7.1f}-{seg[-1]['game_time']:7.1f} n={len(seg):5d} peak_h={max(r['speed_h'] for r in seg):6.1f} jumps={jumps} slides={slides} ducks={sum(1 for k in range(1,len(seg)) if seg[k]['duck'] and not seg[k-1]['duck'])} side_inputs={sum(1 for k in range(1,len(seg)) if seg[k]['side_move']!=0 and seg[k-1]['side_move']==0)} ms={sorted(set(round(r['max_speed']) for r in seg))[-1]}")
