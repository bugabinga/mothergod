import os, math, collections, glob, random, json, sqlite3, time, urllib.request

# ============ pipeline (from our evolved compressor) ============
def sd(d,k): return bytes(list(d[:k])+[(d[i]-d[i-k])%256 for i in range(k,len(d))])
def usd(d,k):
    o=list(d[:k])
    for i in range(k,len(d)): o.append((d[i]+o[i-k])%256)
    return bytes(o)
FILTERS={'id':(lambda d:d, lambda d:d)}
for K in (1,2,4,8,200):
    FILTERS[f'd{K}']=((lambda k:(lambda d:sd(d,k), lambda d:usd(d,k)))(K))

GENOME=dict(min_match=4,cand=64,lazy=256,maxlen=32767,W=32767,inc=16,lim=65536)
def CTX(b1,b2): return b1&0xF0
N_CTX=256

def lz(data,g):
    idx=collections.defaultdict(list); n=len(data); toks=[]
    cand,ml,W,lt=g['cand'],g['maxlen'],g['W'],g['lazy']
    def find(i):
        best,bj=0,0
        for j in reversed(idx.get(data[i:i+3],[])[-cand:]):
            if i-j>W: break
            l=0
            while l<ml and i+l<n and data[j+l]==data[i+l]: l+=1
            if l>best: best,bj=l,j
            if best>=ml: break
        return best,i-bj
    i=0
    while i<n:
        l1,o1=find(i); idx[data[i:i+3]].append(i)
        if l1>=4 and lt and l1<lt and i+1<n:
            l2,_=find(i+1)
            if l2>l1: toks.append(('l',data[i],i)); i+=1; continue
        if l1>=g['min_match']:
            for k in range(i+1,i+l1): idx[data[k:k+3]].append(k)
            toks.append(('m',l1,o1,i)); i+=l1
        else: toks.append(('l',data[i],i)); i+=1
    return toks

def bkt(v): return v.bit_length()-1
class M:
    __slots__=('f','tot','inc','lim')
    def __init__(s,n,inc,lim): s.f=[1]*n; s.tot=n; s.inc=inc; s.lim=lim
    def cost(s,sym):
        c=-math.log2(s.f[sym]/s.tot); s.f[sym]+=s.inc; s.tot+=s.inc
        if s.tot>s.lim:
            s.tot=0
            for i in range(len(s.f)): s.f[i]=(s.f[i]+1)>>1; s.tot+=s.f[i]
        return c

def ac_stage(toks,data,g,ctx_fn):
    tab={}; om=M(17,g['inc'],g['lim']); bits=0.0
    for t in toks:
        pos=t[-1]
        c=ctx_fn(data[pos-1] if pos else 0, data[pos-2] if pos>1 else 0)
        m=tab.get(c)
        if m is None: m=tab[c]=M(272,g['inc'],g['lim'])
        if t[0]=='l': bits+=m.cost(t[1])
        else:
            bits+=m.cost(256+bkt(t[1]))+bkt(t[1])
            bits+=om.cost(bkt(t[2]))+bkt(t[2])
    return bits

def benchmark(genome,ctx_fn,filters):
    res={}
    for name,d in CORP.items():
        # cheap filter pre-selection
        scores=[]
        for fn,(f,_) in filters.items():
            fd=f(d[:6000]); toks=lz(fd,dict(genome,cand=8,maxlen=255,lazy=0))
            cnt=collections.Counter(t[1] if t[0]=='l' else 256 for t in toks)
            tot=sum(cnt.values())
            scores.append((sum(v*-math.log2(v/tot) for v in cnt.values())+len(toks),fn))
        scores.sort()
        best=None
        for fn in {'id',scores[0][1]}:
            fd=filters[fn][0](d)
            b=ac_stage(lz(fd,genome),fd,genome,ctx_fn)
            if best is None or b<best: best=b
        res[name]=best/len(d)
    res['TOTAL']=sum(res.values())
    return res

# ============ corpus ============
random.seed(11); N=10000
def mk():
    c={}
    docs=[]; seen=set()
    for p in sorted(glob.glob('/usr/share/doc/*/copyright'))[:120]:
        b=open(p,'rb').read()
        if b[:200] not in seen: seen.add(b[:200]); docs.append(b)
    c['text']=b'\n'.join(docs)[:N]
    c['code']=b''.join(open(p,'rb').read() for p in sorted(glob.glob('/usr/lib/python3*/[a-z]*.py'))[:30])[:N]
    c['elf']=open('/usr/lib/x86_64-linux-gnu/libc.so.6','rb').read()[200000:200000+N]
    aud=[]
    for i in range(N//2):
        v=int(2500*math.sin(i/37)+1500*math.sin(i/11)+200*random.gauss(0,1))&0xffff
        aud+=[v&255,v>>8]
    c['audio16']=bytes(aud)
    img=[]
    for y in range(N//200+1):
        for x in range(200):
            img.append(int(90+70*math.sin(x/31)+50*math.sin(y/23)+8*random.gauss(0,1))&255)
    c['image']=bytes(img[:N])
    c['random']=os.urandom(N)
    return c
CORP=mk()

# ============ the researcher ============
SYSTEM="""You are a compression researcher improving a codec: per-file filter bank -> LZ77 -> adaptive arithmetic coder with contexted literal models.
Respond ONLY with JSON, one experiment, no prose. Schemas:
{"idea":"<why>","kind":"param","changes":{"inc":12}}                       # keys: min_match 3-6, cand 8-128, lazy 0-256, maxlen<=32767, W<=65535, inc 4-64, lim 4096-131072
{"idea":"<why>","kind":"filter","name":"x","code":"def filt(d):\\n ...return bytes\\ndef unfilt(d):\\n ...return bytes"}   # MUST be exactly invertible
{"idea":"<why>","kind":"context","code":"N_CTX=64\\ndef ctx(b1,b2):\\n return ...  # int in [0,N_CTX)"}  # b1=prev byte, b2=byte before
Propose something NOT already in the journal. Exploit weaknesses visible in the scores."""

def llm_call(prompt):
    key=os.environ.get('ANTHROPIC_API_KEY')
    if key:
        req=urllib.request.Request('https://api.anthropic.com/v1/messages',
            data=json.dumps({'model':'claude-sonnet-4-6','max_tokens':1200,'system':SYSTEM,
                'messages':[{'role':'user','content':prompt}]}).encode(),
            headers={'content-type':'application/json','x-api-key':key,'anthropic-version':'2023-06-01'})
        r=json.loads(urllib.request.urlopen(req,timeout=120).read())
        return ''.join(b.get('text','') for b in r['content'])
    print('  [no API key -> SIMULATED researcher fixture]')
    return FIXTURES.pop(0)

FIXTURES=[  # stand-ins for live LLM output, same schema it would produce
 '{"idea":"slower adaptation may reduce noise-fitting on elf/random","kind":"param","changes":{"inc":8}}',
 '{"idea":"sort bytes ascending, huge runs for LZ","kind":"filter","name":"sortbytes","code":"def filt(d):\\n return bytes(sorted(d))\\ndef unfilt(d):\\n return d"}',
 '{"idea":"linear extrapolation predictor for smooth signals: p=2*prev-prev2","kind":"filter","name":"lin1","code":"def filt(d):\\n o=list(d[:2])\\n for i in range(2,len(d)): o.append((d[i]-(2*d[i-1]-d[i-2]))%256)\\n return bytes(o)\\ndef unfilt(d):\\n o=list(d[:2])\\n for i in range(2,len(d)): o.append((d[i]+2*o[i-1]-o[i-2])%256)\\n return bytes(o)"}',
 '{"idea":"two-byte partial context: high nibble of prev + top 2 bits of prev2","kind":"context","code":"N_CTX=64\\ndef ctx(b1,b2):\\n return ((b1>>4)<<2)|(b2>>6)"}',
 '{"idea":"with richer contexts, even slower learning","kind":"param","changes":{"lim":32768}}',
]

def verify_filter(f,uf):
    for d in (CORP['audio16'][:3000], CORP['text'][:3000]):
        if uf(f(d))!=d: return False
    return True

# ============ research loop ============
best=benchmark(GENOME,CTX,FILTERS)
journal=[]
print(f'baseline TOTAL {best["TOTAL"]:.3f}  {{'+', '.join(f"{k}:{v:.2f}" for k,v in best.items() if k!="TOTAL")+'}}\n')
state=dict(genome=dict(GENOME),ctx=CTX,filters=dict(FILTERS))

for it in range(1,6):
    prompt=(f'Current scores (bits/byte): {json.dumps({k:round(v,3) for k,v in best.items()})}\n'
            f'Genome: {json.dumps(state["genome"])}\nFilters: {list(state["filters"])}\n'
            f'Journal: {json.dumps(journal)}\nPropose one experiment.')
    try:
        raw=llm_call(prompt).strip().removeprefix('```json').removeprefix('```').removesuffix('```')
        exp=json.loads(raw)
        g,cf,fl=dict(state['genome']),state['ctx'],dict(state['filters'])
        if exp['kind']=='param': g.update({k:int(v) for k,v in exp['changes'].items() if k in g})
        elif exp['kind']=='filter':
            ns={'__builtins__':__builtins__}; exec(exp['code'],ns)
            if not verify_filter(ns['filt'],ns['unfilt']): raise ValueError('NOT INVERTIBLE — rejected (fitness gaming guard)')
            fl[exp['name']]=(ns['filt'],ns['unfilt'])
        elif exp['kind']=='context':
            ns={'__builtins__':__builtins__}; exec(exp['code'],ns)
            probe={ns['ctx'](a,b) for a in range(0,256,7) for b in range(0,256,11)}
            if max(probe)>=ns['N_CTX'] or min(probe)<0: raise ValueError('ctx out of range')
            cf=ns['ctx']
        r=benchmark(g,cf,fl)
        delta=r['TOTAL']-best['TOTAL']; ok=delta<0
        print(f'it{it} [{exp["kind"]:7}] {exp["idea"][:58]:<58} {r["TOTAL"]:.3f} ({delta:+.3f}) {"ACCEPT" if ok else "reject"}')
        journal.append({'kind':exp['kind'],'idea':exp['idea'],'delta':round(delta,3),'accepted':ok})
        if ok: best=r; state=dict(genome=g,ctx=cf,filters=fl)
    except Exception as e:
        print(f'it{it} FAILED: {e}')
        journal.append({'kind':'error','idea':str(e)[:80],'accepted':False})

print(f'\nfinal TOTAL {best["TOTAL"]:.3f}  {{'+', '.join(f"{k}:{v:.2f}" for k,v in best.items() if k!="TOTAL")+'}}')
print('journal:', json.dumps(journal,indent=1))
