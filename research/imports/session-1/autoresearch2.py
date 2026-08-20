import os, sys, math, json, random, collections
exec(open('autoresearch.py').read().split('# ============ corpus')[0])  # sd,usd,FILTERS,CTX,M,bkt + base funcs (overridden below)
from corpus import make_corpus
import zstandard as zs
Z=zs.ZstdCompressor(level=19)
BIG,_=make_corpus(60000)
TRAIN=['text','code','elf','sqlite','audio16','image','iid-H2','markov-H8/2','zipped']
VAL=['log','json','pem-certs','iid-H4','b64-text','iid-H8']
S=10000; STATE='research_state.json'; PROG='progress.jsonl'

# ---- pipeline v3: optimal parse + 3-slot rep cache + model mixing (researcher patch, it9) ----
def lz(data,g):
    idx=collections.defaultdict(list); n=len(data); toks=[]; last=0
    cand,ml,W,lt=g['cand'],g['maxlen'],g['W'],g['lazy']
    def mlen(i,dist):
        l=0
        while l<ml and i+l<n and i-dist>=0 and data[i-dist+l]==data[i+l]: l+=1
        return l
    def find(i):
        best,bj=0,0
        for j in reversed(idx.get(data[i:i+3],[])[-cand:]):
            if i-j>W: break
            l=mlen(i,i-j)
            if l>best: best,bj=l,i-j
            if best>=ml: break
        return best,bj
    i=0
    while i<n:
        l1,o1=find(i); idx[data[i:i+3]].append(i)
        rep=mlen(i,last) if (g.get('rep') and last) else 0
        if l1>=4 and lt and l1<lt and i+1<n and rep<l1:
            l2,_=find(i+1)
            if l2>l1: toks.append(('l',data[i],i)); i+=1; continue
        if rep>=4 and rep>=l1-1:
            for k in range(i+1,i+rep): idx[data[k:k+3]].append(k)
            toks.append(('r',rep,i,0)); i+=rep
        elif l1>=g['min_match']:
            for k in range(i+1,i+l1): idx[data[k:k+3]].append(k)
            toks.append(('m',l1,o1,i)); last=o1; i+=l1
        else: toks.append(('l',data[i],i)); i+=1
    return toks

_BOUND=[4,5,6,8,10,12,16,20,24,32,40,48,63]
def lz_opt(data,g):
    gt=lz(data,dict(g,rep=0))
    lh=collections.Counter(); lb=collections.Counter(); ob=collections.Counter()
    ln=[collections.Counter() for _ in range(16)]
    for t in gt:
        if t[0]=='l':
            lh[t[1]]+=1; ln[data[t[2]-1]>>4 if t[2] else 0][t[1]]+=1
        else: lb[bkt(t[1])]+=1; ob[bkt(t[2])]+=1
    def tab(c,size):
        tot=sum(c.values())+size
        return [-math.log2((c.get(k,0)+1)/tot) for k in range(size)]
    litc=tab(lh,256); lenc=tab(lb,16); offc=tab(ob,16)
    litcn=[tab(c,256) for c in ln] if g.get('dpctx') else None
    rp=2.5
    n=len(data); INF=float('inf'); ml=g['maxlen']; W=g['W']; cand=min(g['cand'],g.get('dpcand',24))
    REP=g.get('reppar'); RMIN=2 if g.get('rep2') else 3
    def mlen(i,dist):
        l=0
        while l<ml and i+l<n and i-dist>=0 and data[i-dist+l]==data[i+l]: l+=1
        return l
    def parse(litc,litcn,lenc,offc,rp):
        dp=[INF]*(n+1); dp[0]=0.0; par=[None]*(n+1)
        rc=[None]*(n+1); rc[0]=(1,4,8)
        idx=collections.defaultdict(list); carry_l=0; carry_o=0
        for i in range(n):
            idx[data[i:i+3]].append(i)
            if dp[i]==INF: continue
            lc = litcn[data[i-1]>>4 if i else 0][data[i]] if litcn else litc[data[i]]
            c=dp[i]+lc
            if c<dp[i+1]: dp[i+1]=c; par[i+1]=('l',1,0); rc[i+1]=rc[i]
            if REP:
                cache=rc[i]
                for slot,o in enumerate(cache):
                    lr=mlen(i,o)
                    if lr>=RMIN:
                        nc=(o,)+tuple(x for x in cache if x!=o)[:2]
                        Ls=[x for x in _BOUND if x<=lr]+([lr] if lr not in _BOUND else [])
                        if RMIN==2 and lr>=2 and 2 not in Ls: Ls=[2]+Ls
                        for L in Ls:
                            if L<RMIN: continue
                            cst=dp[i]+lenc[bkt(L)]+bkt(L)+rp
                            if cst<dp[i+L]: dp[i+L]=cst; par[i+L]=('r',L,slot); rc[i+L]=nc
            if carry_l>=64:
                l1,o1=carry_l,carry_o; carry_l-=1
            else:
                best,bj=0,0
                for j in reversed(idx.get(data[i:i+3],[])[-cand:]):
                    if j==i or i-j>W: continue
                    l=mlen(i,i-j)
                    if l>best: best,bj=l,i-j
                    if best>=ml: break
                l1,o1=best,bj
                if l1>=64: carry_l,carry_o=l1-1,o1
            if l1==3 and o1 and o1<4096 and g.get('mm3'):
                c=dp[i]+lenc[bkt(3)]+bkt(3)+offc[bkt(o1)]+bkt(o1)
                if c<dp[i+3]: dp[i+3]=c; par[i+3]=('m',3,o1); rc[i+3]=(o1,)+rc[i][:2]
            if l1>=4:
                oc=offc[bkt(o1)]+bkt(o1)
                nc=(o1,)+tuple(x for x in rc[i] if x!=o1)[:2]
                if l1<64:
                    for L in _BOUND:
                        if L>l1: break
                        c=dp[i]+lenc[bkt(L)]+bkt(L)+oc
                        if c<dp[i+L]: dp[i+L]=c; par[i+L]=('m',L,o1); rc[i+L]=nc
                c=dp[i]+lenc[bkt(l1)]+bkt(l1)+oc
                if c<dp[i+l1]: dp[i+l1]=c; par[i+l1]=('m',l1,o1); rc[i+l1]=nc
        toks=[]; i=n
        while i>0:
            k,L,o=par[i]; i-=L
            if k=='l': toks.append(('l',data[i],i))
            elif k=='r': toks.append(('r',L,i,o))
            else: toks.append(('m',L,o,i))
        toks.reverse()
        return toks
    toks=parse(litc,litcn,lenc,offc,rp)
    if g.get('opt2'):
        lh=collections.Counter(); lb=collections.Counter(); ob=collections.Counter()
        ln=[collections.Counter() for _ in range(16)]; nrep=0
        for t in toks:
            if t[0]=='l':
                lh[t[1]]+=1; ln[data[t[2]-1]>>4 if t[2] else 0][t[1]]+=1
            elif t[0]=='r': lb[bkt(t[1])]+=1; nrep+=1
            else: lb[bkt(t[1])]+=1; ob[bkt(t[2])]+=1
        litc=tab(lh,256); lenc=tab(lb,16); offc=tab(ob,16)
        litcn=[tab(c,256) for c in ln] if g.get('dpctx') else None
        rp=-math.log2((nrep+1)/(len(toks)+2))
        toks=parse(litc,litcn,lenc,offc,rp)
    if g.get('rep3') and not REP:
        cache=[0,0,0]; out=[]
        for t in toks:
            if t[0]=='m':
                _,L,o,pos=t
                if o in cache:
                    sl=cache.index(o); out.append(('r',L,pos,sl))
                    cache.pop(sl); cache.insert(0,o)
                else:
                    out.append(t); cache.pop(); cache.insert(0,o)
            else: out.append(t)
        toks=out
    return toks

class Lit:  # literal/length coder: mixed experts (ctx, order-0, optional hashed order-2)
    def __init__(s,g):
        s.tab={}; s.g=g; s.mix=g.get('mix')
        s.m0=M(272,g['inc'],g['lim']); s.w=[1.0,1.0,1.0,1.0]
        s.o2=g.get('o2'); s.t2={}; s.lm=M(272,g['inc'],g['lim'])
    def cost(s,c,sym,b1=0,b2=0,pos=0):
        if s.g.get('lensep') and sym>=256: return s.lm.cost(sym)
        m=s.tab.get(c)
        if m is None: m=s.tab[c]=M(272,s.g['inc'],s.g['lim'])
        if not s.mix: return m.cost(sym)
        p1=m.f[sym]/m.tot; p2=s.m0.f[sym]/s.m0.tot
        if s.g.get('backoff'):
            nn=max(m.tot-272,0); K=s.g.get('bk',192)
            pm=(nn*p1+K*p2)/(nn+K)
        elif s.g.get('mix2'):
            ps=[p1,p2]
            if s.o2:
                h=((b1<<8)|b2)&s.g.get('o2m',0xFFF)
                m2=s.t2.get(h)
                if m2 is None: m2=s.t2[h]=M(272,s.g['inc'],s.g['lim'])
                ps.append(m2.f[sym]/m2.tot); m2.cost(sym)
            if s.g.get('algn'):
                ha=0x10000|((pos&3)<<4)|(b1>>4)
                m4=s.t2.get(ha)
                if m4 is None: m4=s.t2[ha]=M(272,s.g['inc'],s.g['lim'])
                ps.append(m4.f[sym]/m4.tot); m4.cost(sym)
            W=sum(s.w[:len(ps)]); pm=sum(w*p for w,p in zip(s.w,ps))/W
            lr=0.05; q=max(pm,1e-9)
            for k in range(len(ps)):
                s.w[k]=min(max(s.w[k]*math.exp(lr*(ps[k]-pm)/q),1e-4),1e4)
        else:
            w1,w2=s.w; pm=(w1*p1+w2*p2)/(w1+w2)
            s.w[0]=w1*(p1**0.06); s.w[1]=w2*(p2**0.06)
            if s.w[0]+s.w[1]<1e-6: s.w=[w*1e6 for w in s.w]
        m.cost(sym); s.m0.cost(sym)
        return -math.log2(pm)

def ac_stage(toks,data,g,cf):
    lit=Lit(g); oms=[M(18,g['inc'],g['lim']),M(18,g['inc'],g['lim'])]; bits=0.0; after_m=False
    for t in toks:
        pos=t[-2] if t[0]=='r' else t[-1]
        c=cf(data[pos-1] if pos else 0, data[pos-2] if pos>1 else 0)
        if g.get('stctx'): c=c+(4096 if after_m else 0)
        if g.get('parctx'): c=c+131072*(pos&1)
        after_m=(t[0]!='l')
        B1=data[pos-1] if pos else 0; B2=data[pos-2] if pos>1 else 0
        if t[0]=='l': bits+=lit.cost(c,t[1],B1,B2,pos)
        elif t[0]=='r':
            om=oms[1 if (g.get('offlen') and t[1]>=16) else 0]
            bits+=lit.cost(c,256+bkt(t[1]),B1,B2,pos)+bkt(t[1])+om.cost(15+t[3])
        else:
            om=oms[1 if (g.get('offlen') and t[1]>=16) else 0]
            bits+=lit.cost(c,256+bkt(t[1]),B1,B2,pos)+bkt(t[1])
            bits+=om.cost(bkt(t[2]))+bkt(t[2])
    return bits

def ac_only(data,g,cf):
    lit=Lit(g); m0=M(272,g['inc'],g['lim']); b1=0.0; b0=0.0
    for pos in range(len(data)):
        c=cf(data[pos-1] if pos else 0, data[pos-2] if pos>1 else 0)
        if g.get('parctx'): c=c+131072*(pos&1)
        b1+=lit.cost(c,data[pos],data[pos-1] if pos else 0,data[pos-2] if pos>1 else 0,pos); b0+=m0.cost(data[pos])
    return min(b0,b1)+1

def compress(d,g,cf,fl):
    scores=[]
    for fn,(f,_) in fl.items():
        fd=f(d[:6000])
        if g.get('selo1'):
            pair=collections.Counter(zip(fd,fd[1:])); pv=collections.Counter(fd[:-1])
            h=-sum(v*math.log2(v/pv[a]) for (a,b),v in pair.items())
            scores.append((h,fn))
        else:
            toks=lz(fd,dict(g,cand=8,maxlen=255,lazy=0,rep=0))
            cnt=collections.Counter(t[1] if t[0]=='l' else 256 for t in toks); tot=sum(cnt.values())
            scores.append((sum(v*-math.log2(v/tot) for v in cnt.values())+len(toks),fn))
    scores.sort()
    arms={'id',scores[0][1]}|({scores[1][1]} if g.get('selo1') else set())
    if g.get('seltrial'): arms|={scores[2][1]} if len(scores)>2 else set()
    best=None
    for fn in arms:
        fd=fl[fn][0](d)
        toks=lz_opt(fd,g) if g.get('opt') else lz(fd,g)
        b=ac_stage(toks,fd,g,cf)
        if g.get('ac_arm'): b=min(b,ac_only(fd,g,cf)+2)
        if best is None or b<best: best=b
    if g.get('fallback'): best=min(best,8*len(d)+1)
    return best/len(d)

# ---- infra ----
def load():
    if os.path.exists(STATE): return json.load(open(STATE))
def save(st): json.dump(st,open(STATE,'w'),indent=1)
def build(st):
    fl=dict(FILTERS)
    for n,code in st['filters'].items():
        ns={}; exec(code,ns); fl[n]=(ns['filt'],ns['unfilt'])
    cf=CTX
    if st['ctx_code']:
        ns={}; exec(st['ctx_code'],ns); cf=ns['ctx']
    cx={}
    for n,code in st['corpus'].items():
        ns={'random':random,'math':math}; exec(code,ns); cx[n]=ns['gen'](60000,random.Random(5))
    return st['genome'],cf,fl,cx
def bench(st,which):
    g,cf,fl,cx=build(st); it=st['it']; out={}
    pool=dict(BIG); pool.update(cx)
    keys=TRAIN+list(cx) if which=='train' else VAL
    for k in keys:
        d=pool[k]
        off=(it*7919)%(len(d)-S) if which=='train' else len(d)-S
        out[k]=compress(d[off:off+S],g,cf,fl)
    out['TOTAL']=sum(out.values())
    return out

def run_exp(exp):
    st=load(); st['it']+=1
    trial=json.loads(json.dumps(st))
    if exp['kind']=='param': trial['genome'].update(exp['changes'])
    elif exp['kind']=='filter':
        ns={}; exec(exp['code'],ns)
        for d in (BIG['audio16'][:3000],BIG['text'][:3000]):
            assert ns['unfilt'](ns['filt'](d))==d,'NOT INVERTIBLE'
        trial['filters'][exp['name']]=exp['code']
    elif exp['kind']=='context':
        ns={}; exec(exp['code'],ns)
        assert all(0<=ns['ctx'](a,b)<ns['N_CTX'] for a in range(0,256,7) for b in range(0,256,11))
        trial['ctx_code']=exp['code']
    elif exp['kind']=='corpus':
        ns={'random':random,'math':math}; exec(exp['code'],ns)
        d=ns['gen'](60000,random.Random(5)); zb=len(Z.compress(d))*8/len(d)
        ob=compress(d[:S],*build(st)[:3])
        print(f'  corpus candidate: zstd={zb:.2f} ours={ob:.2f} regret={ob-zb:+.2f}')
        assert zb<7.5,'no structure'; assert ob-zb>0.15,'no regret'
        trial['corpus'][exp['name']]=exp['code']
    base_tr=bench(st,'train'); base_va=bench(st,'val')
    tr=bench(trial,'train'); va=bench(trial,'val')
    dt=tr['TOTAL']-base_tr['TOTAL']; dv=va['TOTAL']-base_va['TOTAL']
    ok=exp['kind']=='corpus' or (dt<0 and dv<0.05) or (dv<-0.05 and dt<0.05)  # rule v2: val wins count too
    print(f"it{st['it']} [{exp['kind']:7}] {exp['idea'][:64]}")
    print(f"  dTrain {dt:+.3f}  dVal {dv:+.3f}  -> {'ACCEPT' if ok else 'REJECT'}")
    st['journal'].append({'it':st['it'],'kind':exp['kind'],'idea':exp['idea'],'dTrain':round(dt,3),'dVal':round(dv,3),'accepted':ok})
    rec=dict(it=st['it'],idea=exp['idea'][:60],kind=exp['kind'],accepted=ok,
             train={k:round(v,3) for k,v in (tr if ok else base_tr).items()},
             val={k:round(v,3) for k,v in (va if ok else base_va).items()})
    if ok:
        trial['journal'],trial['it']=st['journal'],st['it']; st=trial
    open(PROG,'a').write(json.dumps(rec)+'\n')
    save(st)

if __name__=='__main__':
    if sys.argv[1]=='batch':
        for exp in json.load(open(sys.argv[2])): run_exp(exp)
    elif sys.argv[1]=='status':
        st=load(); tr=bench(st,'train'); va=bench(st,'val')
        print('TRAIN',json.dumps({k:round(v,3) for k,v in tr.items()}))
        print('VAL  ',json.dumps({k:round(v,3) for k,v in va.items()}))
