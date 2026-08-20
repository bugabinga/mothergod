// mothergod v0.2 — real-bitstream codec, ported from the evolved research champion
// filters(id/d1/d2/d4/d8/auto-stride, trial-selected) -> LZ(1MB window, lazy, rep3 cache)
// -> adaptive AC: flag model / 4-expert EG-mixed literal coder / SPLIT length+offset models
// INVARIANT: offset buckets 0..=19 (W=1MB); rep coded via flag stream, never offset symbols.
use std::env; use std::fs; use std::process::exit; use std::convert::TryInto;
use std::collections::HashMap; use std::time::Instant;

const TOP:u64=1<<32; const HALF:u64=TOP>>1; const Q1:u64=TOP>>2; const Q3:u64=3*(TOP>>2); const MASK:u64=TOP-1;
const INC:u32=12; const LIM:u32=65536; const W:usize=1<<20; const SCALE:u64=1<<16;

struct Model{f:Vec<u32>,tot:u32}
impl Model{
    fn new(n:usize)->Model{Model{f:vec![1;n],tot:n as u32}}
    fn upd(&mut self,s:usize){
        self.f[s]+=INC; self.tot+=INC;
        if self.tot>LIM{ for v in self.f.iter_mut(){*v=(*v+1)>>1;} self.tot=self.f.iter().sum(); }
    }
    fn enc(&mut self,ac:&mut Enc,s:usize){
        let mut cl=0u64; for i in 0..s{cl+=self.f[i] as u64;}
        ac.code(cl,cl+self.f[s] as u64,self.tot as u64); self.upd(s);
    }
    fn dec(&mut self,ac:&mut Dec)->usize{
        let tg=ac.target(self.tot as u64);
        let mut s=0; let mut cl=0u64;
        while cl+self.f[s] as u64<=tg{cl+=self.f[s] as u64; s+=1;}
        ac.code(cl,cl+self.f[s] as u64,self.tot as u64); self.upd(s); s
    }
}
// DOD arena v0.5 — five experts, two-rate ctx counters, context-sensitive MIX weights
// layout: [0..512) ctx-fast | [512..1024) ctx-slow | [1024] o0 | [1025..1025+4096) o2 | [5121..5185) align
const NB:usize=9281; const O_CF:usize=0; const O_CS:usize=512; const O_O0:usize=1024; const O_O2:usize=1025; const O_AL:usize=5121; const O_WD:usize=5185;
const NE:usize=6; const NW:usize=32;
struct Lit{f:Vec<u32>,tot:Vec<u32>,w:Vec<[f64;NE]>}
impl Lit{
    fn new()->Lit{Lit{f:vec![1u32;NB*256],tot:vec![256u32;NB],w:vec![[1.0;NE];NW]}}
    #[inline]
    fn banks(&self,b1:u8,b2:u8,pos:usize,am:bool,wh:u32)->([usize;NE],usize){
        let c=(b1 as usize)|if am{256}else{0};
        ([O_CF+c, O_CS+c, O_O0,
          O_O2+(((((b1 as u32)<<8)|b2 as u32)&0xFFF) as usize),
          O_AL+(((pos&3)<<4)|((b1>>4) as usize)),
          O_WD+((wh&0xFFF) as usize)],
         ((b1>>4) as usize)|if am{16}else{0})
    }
    #[inline]
    fn cum(&self,bk:&[usize;NE],wi:usize,cum:&mut [u64;257]){
        let w=&self.w[wi]; let ws:f64=w.iter().sum();
        let mut k=[0u64;NE];
        for e in 0..NE{ k[e]=((w[e]/ws)*((1u64<<32) as f64)/(self.tot[bk[e]] as f64)) as u64; }
        let f0=&self.f[bk[0]*256..bk[0]*256+256];
        let f1=&self.f[bk[1]*256..bk[1]*256+256];
        let f2=&self.f[bk[2]*256..bk[2]*256+256];
        let f3=&self.f[bk[3]*256..bk[3]*256+256];
        let f4=&self.f[bk[4]*256..bk[4]*256+256];
        let f5=&self.f[bk[5]*256..bk[5]*256+256];
        let mut acc:u64=0; cum[0]=0;
        for i in 0..256{
            let pm=(k[0]*f0[i] as u64 + k[1]*f1[i] as u64 + k[2]*f2[i] as u64
                  + k[3]*f3[i] as u64 + k[4]*f4[i] as u64 + k[5]*f5[i] as u64)>>16;
            acc+=pm+1; cum[i+1]=acc;
        }
    }
    #[inline]
    fn upd(&mut self,bk:&[usize;NE],wi:usize,s:usize){
        let mut ps=[0f64;NE];
        for e in 0..NE{ ps[e]=(self.f[bk[e]*256+s] as f64)/(self.tot[bk[e]] as f64); }
        let w=&mut self.w[wi]; let ws:f64=w.iter().sum();
        let pm:f64=(0..NE).map(|e|w[e]*ps[e]).sum::<f64>()/ws;
        let q=pm.max(1e-9); let lr=0.05;
        for e in 0..NE{ w[e]=(w[e]*((lr*(ps[e]-pm)/q).exp())).clamp(1e-4,1e4); }
        for e in 0..NE{
            let b=bk[e];
            let (inc,lim)=if e==0{(32u32,6144u32)}else{(INC,LIM)}; // ctx-fast: high rate, short memory
            self.f[b*256+s]+=inc; self.tot[b]+=inc;
            if self.tot[b]>lim{
                let mut t=0u32;
                for v in self.f[b*256..b*256+256].iter_mut(){*v=(*v+1)>>1; t+=*v;}
                self.tot[b]=t;
            }
        }
    }
}
#[inline]
fn whup(wh:u32,b:u8)->u32{ if b.is_ascii_alphanumeric(){ wh.wrapping_mul(61).wrapping_add(b as u32) } else {0} }
fn bkt(v:u32)->usize{ (32-v.leading_zeros()-1) as usize }

struct Enc{lo:u64,hi:u64,pend:u32,out:Vec<u8>,bit:u8,nb:u8}
impl Enc{
    fn new()->Enc{Enc{lo:0,hi:MASK,pend:0,out:vec![],bit:0,nb:0}}
    fn emit(&mut self,b:u8){ self.bit=(self.bit<<1)|b; self.nb+=1; if self.nb==8{self.out.push(self.bit); self.bit=0; self.nb=0;} }
    fn code(&mut self,cl:u64,ch:u64,tot:u64){
        let rng=self.hi-self.lo+1;
        self.hi=self.lo+rng*ch/tot-1; self.lo=self.lo+rng*cl/tot;
        loop{
            if self.hi<HALF{ self.emit(0); for _ in 0..self.pend{self.emit(1);} self.pend=0; }
            else if self.lo>=HALF{ self.emit(1); for _ in 0..self.pend{self.emit(0);} self.pend=0; self.lo-=HALF; self.hi-=HALF; }
            else if self.lo>=Q1 && self.hi<Q3{ self.pend+=1; self.lo-=Q1; self.hi-=Q1; }
            else{break;}
            self.lo=(self.lo<<1)&MASK; self.hi=((self.hi<<1)|1)&MASK;
        }
    }
    fn bits(&mut self,v:u32,n:usize){ for k in (0..n).rev(){ let b=((v>>k)&1) as u64; self.code(b*(SCALE/2),(b+1)*(SCALE/2),SCALE); } }
    fn finish(mut self)->Vec<u8>{
        self.pend+=1;
        if self.lo<Q1{ self.emit(0); for _ in 0..self.pend{self.emit(1);} } else { self.emit(1); for _ in 0..self.pend{self.emit(0);} }
        while self.nb!=0{ self.emit(0); } self.out
    }
}
struct Dec<'a>{lo:u64,hi:u64,val:u64,data:&'a[u8],pos:usize}
impl<'a> Dec<'a>{
    fn new(d:&'a[u8])->Dec<'a>{ let mut s=Dec{lo:0,hi:MASK,val:0,data:d,pos:0};
        for _ in 0..32{ let b=s.bit(); s.val=(s.val<<1)|b as u64; } s }
    fn bit(&mut self)->u8{ let b=if self.pos>>3<self.data.len(){(self.data[self.pos>>3]>>(7-(self.pos&7)))&1}else{0}; self.pos+=1; b }
    fn target(&self,tot:u64)->u64{ let rng=self.hi-self.lo+1; ((self.val-self.lo+1)*tot-1)/rng }
    fn code(&mut self,cl:u64,ch:u64,tot:u64){
        let rng=self.hi-self.lo+1;
        self.hi=self.lo+rng*ch/tot-1; self.lo=self.lo+rng*cl/tot;
        loop{
            if self.hi<HALF{}
            else if self.lo>=HALF{ self.lo-=HALF; self.hi-=HALF; self.val-=HALF; }
            else if self.lo>=Q1 && self.hi<Q3{ self.lo-=Q1; self.hi-=Q1; self.val-=Q1; }
            else{break;}
            self.lo=(self.lo<<1)&MASK; self.hi=((self.hi<<1)|1)&MASK;
            let b=self.bit(); self.val=((self.val<<1)|b as u64)&MASK;
        }
    }
    fn bits(&mut self,n:usize)->u32{ let mut v=0u32;
        for _ in 0..n{ let tg=self.target(SCALE); let b=if tg>=SCALE/2{1u32}else{0};
            self.code(b as u64*(SCALE/2),(b as u64+1)*(SCALE/2),SCALE); v=(v<<1)|b; } v }
}
// ---- filters ----
fn sdelta(d:&[u8],k:usize)->Vec<u8>{ let mut o=d.to_vec(); for i in (k..d.len()).rev(){o[i]=d[i].wrapping_sub(d[i-k]);} o }
fn usdelta(d:&[u8],k:usize)->Vec<u8>{ let mut o=d.to_vec(); for i in k..d.len(){o[i]=o[i].wrapping_add(o[i-k]);} o }
fn o1h(d:&[u8])->f64{
    let mut pair=vec![0u32;65536]; let mut pv=vec![0u32;256];
    for w in d.windows(2){ pair[((w[0] as usize)<<8)|w[1] as usize]+=1; pv[w[0] as usize]+=1; }
    let mut h=0f64;
    for a in 0..256{ if pv[a]==0{continue;}
        for b in 0..256{ let v=pair[(a<<8)|b]; if v>0{ h-=(v as f64)*((v as f64)/(pv[a] as f64)).log2(); } } }
    h/(d.len().max(2) as f64-1.0)
}
fn bcj(d:&[u8],enc:bool)->Vec<u8>{
    let mut b=d.to_vec(); let n=b.len(); let mut i=0usize;
    while i+5<=n{
        if b[i]==0xE8||b[i]==0xE9{
            let v=u32::from_le_bytes(b[i+1..i+5].try_into().unwrap());
            let w=if enc{ v.wrapping_add((i as u32)+5) } else { v.wrapping_sub((i as u32)+5) };
            b[i+1..i+5].copy_from_slice(&w.to_le_bytes());
            i+=5;
        } else { i+=1; }
    }
    b
}
const TPK:[usize;14]=[2,3,4,7,8,12,14,16,24,28,32,56,64,96];
fn tpose(d:&[u8],k:usize)->Vec<u8>{
    let n=d.len(); let mut o=Vec::with_capacity(n);
    for j in 0..k{ let mut i=j; while i<n{ o.push(d[i]); i+=k; } }
    o
}
fn untpose(d:&[u8],k:usize)->Vec<u8>{
    let n=d.len(); let mut o=vec![0u8;n]; let mut p=0;
    for j in 0..k{ let mut i=j; while i<n{ o[i]=d[p]; p+=1; i+=k; } }
    o
}
fn colh(d:&[u8],k:usize)->f64{
    let mut h=0f64; let mut cnt=0usize;
    for j in 0..k{
        let col:Vec<u8>=d[j..].iter().step_by(k).cloned().collect();
        if col.len()<2{continue;}
        let mut pair=std::collections::HashMap::new(); let mut pv=std::collections::HashMap::new();
        for w in col.windows(2){ *pair.entry((w[0],w[1])).or_insert(0u32)+=1; *pv.entry(w[0]).or_insert(0u32)+=1; }
        for ((a,_),v) in pair{ h-=(v as f64)*((v as f64)/(pv[&a] as f64)).log2(); }
        cnt+=col.len()-1;
    }
    h/(cnt.max(1) as f64)
}
fn pick_filters(d:&[u8])->Vec<u8>{ // returns candidate ks (0=id) ranked; auto-stride scan
    let probe=&d[..d.len().min(16384)];
    let mut sc:Vec<(f64,u8)>=vec![(o1h(probe),0)];
    for k in 1..=96u8{ sc.push((o1h(&sdelta(probe,k as usize)),k)); }
    sc.sort_by(|a,b|a.0.partial_cmp(&b.0).unwrap());
    let mut out=vec![sc[0].1, if sc[0].1!=0{0}else{sc[1].1}];
    let e8=d.iter().take(65536).filter(|&&b|b==0xE8||b==0xE9).count();
    if e8*400>d.len().min(65536){ out.push(97); } // x86-ish density -> try BCJ arm
    if d.len()>=4096{
        let base=colh(probe,1); let mut bt=(base,0usize);
        for (ix,&k) in TPK.iter().enumerate(){ let h=colh(probe,k); if h<bt.0{ bt=(h,ix+100); } }
        if bt.1>=100 && bt.0<base-0.35{ out.push(bt.1 as u8); }
    }
    out
}
// ---- LZ ----
enum Tok{L(u8),M(u32,u32),R(u32,u8)}
fn lz(d:&[u8])->Vec<Tok>{
    let n=d.len(); let mut toks=vec![]; let mut reps:[u32;3]=[1,4,8];
    let hbits=17; let hmask=(1usize<<hbits)-1;
    let mut head=vec![u32::MAX;1<<hbits]; let mut prev=vec![u32::MAX;n.max(1)];
    let h=|d:&[u8],i:usize|->usize{ if i+3>n{return 0;}
        (((d[i] as usize)<<10) ^ ((d[i+1] as usize)<<5) ^ (d[i+2] as usize))&hmask };
    let mlen=|i:usize,dist:usize|->usize{ if dist==0||dist>i{return 0;}
        let mut l=0; while l<65535 && i+l<n && d[i-dist+l]==d[i+l]{l+=1;} l };
    let find=|head:&Vec<u32>,prev:&Vec<u32>,i:usize|->(usize,usize){
        let mut best=0; let mut bo=0; let mut j=head[h(d,i)]; let mut tries=0;
        while j!=u32::MAX && tries<128{
            let dist=i-(j as usize);
            if dist>W{break;}
            if dist>0{ let l=mlen(i,dist); if l>best{best=l;bo=dist;} }
            j=prev[j as usize]; tries+=1;
        } (best,bo) };
    let mut i=0usize;
    while i<n{
        let hh=h(d,i); prev[i]=head[hh]; head[hh]=i as u32;
        let mut br=0usize; let mut bs=0u8;
        for s in 0..3{ let l=mlen(i,reps[s] as usize); if l>br{br=l; bs=s as u8;} }
        let (mut l1,mut o1)=find(&head,&prev,i);
        if l1>=4 && l1<256 && br+1<l1 && i+1<n{ // lazy
            let hh2=h(d,i+1);
            let saved=head[hh2];
            let (l2,_)=find(&head,&prev,i+1);
            let _=saved;
            if l2>l1{ toks.push(Tok::L(d[i])); i+=1; continue; }
        }
        if br>=2 && br+1>=l1{
            for k in i+1..i+br{ let hk=h(d,k); prev[k]=head[hk]; head[hk]=k as u32; }
            toks.push(Tok::R(br as u32,bs));
            let o=reps[bs as usize]; if bs>0{ for s in (1..=bs as usize).rev(){reps[s]=reps[s-1];} reps[0]=o; }
            i+=br;
        } else if l1>=4{
            for k in i+1..i+l1{ let hk=h(d,k); prev[k]=head[hk]; head[hk]=k as u32; }
            toks.push(Tok::M(l1 as u32,o1 as u32));
            reps[2]=reps[1]; reps[1]=reps[0]; reps[0]=o1 as u32;
            i+=l1;
        } else { toks.push(Tok::L(d[i])); i+=1; }
        let _=&mut l1; let _=&mut o1;
    }
    toks
}
// ---- optimal parse: priced DP with in-DP rep cache, 2-round price iteration ----
const BOUND:[u32;13]=[4,5,6,8,10,12,16,20,24,32,40,48,63];
fn lz_opt(d:&[u8])->Vec<Tok>{
    let n=d.len();
    if n<64{ return lz(d); }
    // pass 1: greedy for prices
    let gt=lz(d);
    let mut lh=vec![1u32;16*256]; let mut lb=vec![1u32;17]; let mut ob=vec![1u32;21]; let mut nrep=1u32;
    let mut pos=0usize;
    for t in &gt{ match t{
        Tok::L(b)=>{ let c=if pos>0{(d[pos-1]>>4) as usize}else{0}; lh[c*256+*b as usize]+=1; pos+=1; }
        Tok::M(l,o)=>{ lb[bkt(*l)]+=1; ob[bkt(*o)]+=1; pos+=*l as usize; }
        Tok::R(l,_)=>{ lb[bkt(*l)]+=1; nrep+=1; pos+=*l as usize; }
    }}
    let price=|f:u32,tot:u32|->f64{ -( (f as f64)/(tot as f64) ).log2() };
    let mk_litc=|lh:&Vec<u32>|->Vec<f64>{
        let mut litc=vec![0f64;16*256];
        for c in 0..16{ let tot:u32=lh[c*256..c*256+256].iter().sum();
            for i in 0..256{ litc[c*256+i]=price(lh[c*256+i],tot); } }
        litc };
    let mk16=|v:&Vec<u32>|->Vec<f64>{ let tot:u32=v.iter().sum(); v.iter().map(|&f|price(f,tot)).collect() };
    let mut litc=mk_litc(&lh); let mut lenc=mk16(&lb); let mut offc=mk16(&ob);
    let mut rp=price(nrep,(gt.len() as u32)+2)+1.6; // + slot bits
    // hash chains
    let hbits=17; let hmask=(1usize<<hbits)-1;
    let h=|i:usize|->usize{ if i+3>n{return 0;}
        (((d[i] as usize)<<10) ^ ((d[i+1] as usize)<<5) ^ (d[i+2] as usize))&hmask };
    let mlen=|i:usize,dist:usize|->usize{ if dist==0||dist>i{return 0;}
        let mut l=0; while l<65535 && i+l<n && d[i-dist+l]==d[i+l]{l+=1;} l };
    let mut toks:Vec<Tok>=vec![];
    for round in 0..2{
        let mut dp=vec![f64::INFINITY;n+1]; dp[0]=0.0;
        // par: kind(0=lit,1=match,2=rep) , len, val(off or slot)
        let mut par=vec![(0u8,0u32,0u32);n+1];
        let mut rc=vec![[0u32;3];n+1]; rc[0]=[1,4,8];
        let mut head=vec![u32::MAX;1<<hbits]; let mut prev=vec![u32::MAX;n];
        let mut carry_l=0usize; let mut carry_o=0usize;
        for i in 0..n{
            let hh=h(i); prev[i]=head[hh]; head[hh]=i as u32;
            if dp[i].is_infinite(){ continue; }
            let c=if i>0{(d[i-1]>>4) as usize}else{0};
            let lc=litc[c*256+d[i] as usize]+1.0; // +flag bit approx
            if dp[i]+lc<dp[i+1]{ dp[i+1]=dp[i]+lc; par[i+1]=(0,1,0); rc[i+1]=rc[i]; }
            // rep candidates
            let cache=rc[i];
            for slot in 0..3{
                let o=cache[slot]; let lr=mlen(i,o as usize);
                if lr>=2{
                    let mut nc=[o,0,0]; let mut w=1;
                    for x in cache{ if x!=o && w<3{ nc[w]=x; w+=1; } }
                    let relax=|dp:&mut Vec<f64>,par:&mut Vec<(u8,u32,u32)>,rc:&mut Vec<[u32;3]>,ll:u32|{
                        let cst=dp[i]+lenc[bkt(ll)]+bkt(ll) as f64+rp;
                        let j=i+ll as usize;
                        if cst<dp[j]{ dp[j]=cst; par[j]=(2,ll,slot as u32); rc[j]=nc; }
                    };
                    if lr>=2 && 2<4 { relax(&mut dp,&mut par,&mut rc,2.min(lr as u32)); }
                    for &bl in BOUND.iter(){ if (bl as usize)<=lr{ relax(&mut dp,&mut par,&mut rc,bl); } else {break;} }
                    if !BOUND.contains(&(lr as u32)) && lr>2{ relax(&mut dp,&mut par,&mut rc,lr as u32); }
                }
            }
            // normal match
            let (l1,o1);
            if carry_l>=64{ l1=carry_l; o1=carry_o; carry_l-=1; }
            else{
                let mut best=0usize; let mut bo=0usize;
                let mut j=head[hh]; let mut tries=0;
                while j!=u32::MAX && tries<640{
                    let dist=i-(j as usize);
                    if dist>W{break;}
                    if dist>0{ let l=mlen(i,dist); if l>best{best=l;bo=dist;} }
                    j=prev[j as usize]; tries+=1;
                }
                l1=best; o1=bo;
                if l1>=64{ carry_l=l1-1; carry_o=o1; }
            }
            if l1==3 && o1>0 && o1<4096{
                let oc3=offc[bkt(o1 as u32)]+bkt(o1 as u32) as f64+1.6;
                let cst=dp[i]+lenc[bkt(3)]+bkt(3) as f64+oc3;
                if cst<dp[i+3]{
                    let mut nc=[o1 as u32,0,0]; let mut w=1;
                    for x in rc[i]{ if x!=o1 as u32 && w<3{ nc[w]=x; w+=1; } }
                    dp[i+3]=cst; par[i+3]=(1,3,o1 as u32); rc[i+3]=nc;
                }
            }
            if l1>=4{
                let oc=offc[bkt(o1 as u32)]+bkt(o1 as u32) as f64+1.6;
                let mut nc=[o1 as u32,0,0]; let mut w=1;
                for x in rc[i]{ if x!=o1 as u32 && w<3{ nc[w]=x; w+=1; } }
                let mut relax=|dp:&mut Vec<f64>,par:&mut Vec<(u8,u32,u32)>,rcv:&mut Vec<[u32;3]>,ll:u32|{
                    let cst=dp[i]+lenc[bkt(ll)]+bkt(ll) as f64+oc;
                    let j=i+ll as usize;
                    if cst<dp[j]{ dp[j]=cst; par[j]=(1,ll,o1 as u32); rcv[j]=nc; }
                };
                if l1<64{ for &bl in BOUND.iter(){ if (bl as usize)<=l1{ relax(&mut dp,&mut par,&mut rc,bl);} else {break;} } }
                relax(&mut dp,&mut par,&mut rc,l1 as u32);
            }
        }
        // reconstruct
        toks.clear(); let mut i=n;
        while i>0{
            let (k,l,v)=par[i]; i-=l as usize;
            match k{ 0=>toks.push(Tok::L(d[i])), 1=>toks.push(Tok::M(l,v)), _=>toks.push(Tok::R(l,v as u8)) }
        }
        toks.reverse();
        if round==0{
            lh=vec![1u32;16*256]; lb=vec![1u32;17]; ob=vec![1u32;21]; nrep=1;
            let mut pos=0usize;
            for t in &toks{ match t{
                Tok::L(b)=>{ let c=if pos>0{(d[pos-1]>>4) as usize}else{0}; lh[c*256+*b as usize]+=1; pos+=1; }
                Tok::M(l,o)=>{ lb[bkt(*l)]+=1; ob[bkt(*o)]+=1; pos+=*l as usize; }
                Tok::R(l,_)=>{ lb[bkt(*l)]+=1; nrep+=1; pos+=*l as usize; }
            }}
            litc=mk_litc(&lh); lenc=mk16(&lb); offc=mk16(&ob);
            rp=price(nrep,(toks.len() as u32)+2)+1.6;
        }
    }
    toks
}
// ---- codec ----
fn encode_body(fd:&[u8])->Vec<u8>{
    let toks=lz_opt(fd);
    let mut lit=Lit::new(); let mut flag=[Model::new(3),Model::new(3)];
    let mut lenm=Model::new(17); let mut offm=Model::new(21); let mut slotm=Model::new(3);
    let mut ac=Enc::new(); let mut cum=[0u64;257];
    let mut pos=0usize; let mut am=false; let mut wh:u32=0;
    for t in &toks{
        let (b1,b2)=(if pos>0{fd[pos-1]}else{0}, if pos>1{fd[pos-2]}else{0});
        let fm=if am{1}else{0};
        match t{
            Tok::L(b)=>{
                flag[fm].enc(&mut ac,0);
                let (bk,wi)=lit.banks(b1,b2,pos,am,wh);
                lit.cum(&bk,wi,&mut cum); let s=*b as usize; let tot=cum[256];
                ac.code(cum[s],cum[s+1],tot); lit.upd(&bk,wi,s);
                wh=whup(wh,*b); pos+=1; am=false;
            }
            Tok::R(l,slot)=>{
                flag[fm].enc(&mut ac,2);
                slotm.enc(&mut ac,*slot as usize);
                let lb=bkt(*l); lenm.enc(&mut ac,lb); ac.bits(*l,lb);
                for kk in pos..pos+*l as usize{ wh=whup(wh,fd[kk]); }
                pos+=*l as usize; am=true;
            }
            Tok::M(l,o)=>{
                flag[fm].enc(&mut ac,1);
                let lb=bkt(*l); lenm.enc(&mut ac,lb); ac.bits(*l,lb);
                let ob=bkt(*o); offm.enc(&mut ac,ob); ac.bits(*o,ob);
                for kk in pos..pos+*l as usize{ wh=whup(wh,fd[kk]); }
                pos+=*l as usize; am=true;
            }
        }
    }
    let mut out=(toks.len() as u32).to_le_bytes().to_vec();
    out.extend(ac.finish()); out
}
fn encode(d:&[u8])->Vec<u8>{
    let cands=pick_filters(d);
    let mut best:Option<(Vec<u8>,u8)>=None;
    for k in cands{
        let fd=if k==0{d.to_vec()}else if k==97{bcj(d,true)}else if k>=100{tpose(d,TPK[(k-100) as usize])}else{sdelta(d,k as usize)};
        let body=encode_body(&fd);
        if best.is_none()||body.len()<best.as_ref().unwrap().0.len(){ best=Some((body,k)); }
    }
    let (body,k)=best.unwrap();
    let mut out=vec![b'M',b'G',1u8,k];
    out.extend((d.len() as u32).to_le_bytes());
    out.extend(body);
    if out.len()>=d.len()+8{
        let mut st=vec![b'M',b'G',0u8,0u8]; st.extend((d.len() as u32).to_le_bytes()); st.extend_from_slice(d); return st;
    }
    out
}
fn decode(z:&[u8])->Vec<u8>{
    assert_eq!(&z[..2],b"MG");
    let mode=z[2]; let k=z[3];
    let n=u32::from_le_bytes(z[4..8].try_into().unwrap()) as usize;
    if mode==0{ return z[8..8+n].to_vec(); }
    let ntok=u32::from_le_bytes(z[8..12].try_into().unwrap()) as usize;
    let mut ac=Dec::new(&z[12..]);
    let mut lit=Lit::new(); let mut flag=[Model::new(3),Model::new(3)];
    let mut lenm=Model::new(17); let mut offm=Model::new(21); let mut slotm=Model::new(3);
    let mut fd:Vec<u8>=Vec::with_capacity(n); let mut cum=[0u64;257];
    let mut reps:[u32;3]=[1,4,8]; let mut am=false; let mut wh:u32=0;
    for _ in 0..ntok{
        let pos=fd.len();
        let (b1,b2)=(if pos>0{fd[pos-1]}else{0}, if pos>1{fd[pos-2]}else{0});
        let fm=if am{1}else{0};
        let f=flag[fm].dec(&mut ac);
        if f==0{
            let (bk,wi)=lit.banks(b1,b2,pos,am,wh);
            lit.cum(&bk,wi,&mut cum); let tot=cum[256];
            let tg=ac.target(tot);
            let mut s=0; while cum[s+1]<=tg{s+=1;}
            ac.code(cum[s],cum[s+1],tot); lit.upd(&bk,wi,s);
            wh=whup(wh,s as u8); fd.push(s as u8); am=false;
        } else if f==2{
            let slot=slotm.dec(&mut ac);
            let lb=lenm.dec(&mut ac); let l=(1u32<<lb)|ac.bits(lb);
            let o=reps[slot];
            if slot>0{ for s in (1..=slot).rev(){reps[s]=reps[s-1];} reps[0]=o; }
            let start=fd.len()-o as usize;
            for kk in 0..l as usize{ let v=fd[start+kk]; wh=whup(wh,v); fd.push(v); }
            am=true;
        } else {
            let lb=lenm.dec(&mut ac); let l=(1u32<<lb)|ac.bits(lb);
            let ob=offm.dec(&mut ac); let o=(1u32<<ob)|ac.bits(ob);
            reps[2]=reps[1]; reps[1]=reps[0]; reps[0]=o;
            let start=fd.len()-o as usize;
            for kk in 0..l as usize{ let v=fd[start+kk]; wh=whup(wh,v); fd.push(v); }
            am=true;
        }
    }
    if k==0{fd}else if k==97{bcj(&fd,false)}else if k>=100{untpose(&fd,TPK[(k-100) as usize])}else{usdelta(&fd,k as usize)}
}
fn encode_par(d:&[u8],nt:usize)->Vec<u8>{
    const BS:usize=1<<21;
    let blocks:Vec<&[u8]>=d.chunks(BS).collect();
    let mut outs:Vec<Vec<u8>>=vec![vec![];blocks.len()];
    let nt=nt.max(1);
    std::thread::scope(|sc|{
        let outs=&mut outs;
        let mut hs=vec![];
        for (chunk_o,chunk_b) in outs.chunks_mut((blocks.len()+nt-1)/nt).zip(blocks.chunks((blocks.len()+nt-1)/nt)){
            let cb:Vec<&[u8]>=chunk_b.to_vec();
            hs.push(sc.spawn(move||{ for (o,b) in chunk_o.iter_mut().zip(cb){ *o=encode(b); } }));
        }
        for h in hs{h.join().unwrap();}
    });
    let mut out=vec![b'M',b'G',b'P',1u8];
    out.extend((blocks.len() as u32).to_le_bytes());
    for o in &outs{ out.extend((o.len() as u32).to_le_bytes()); }
    for o in outs{ out.extend(o); }
    out
}
fn decode_par(z:&[u8],nt:usize)->Vec<u8>{
    assert_eq!(&z[..3],b"MGP");
    let nb=u32::from_le_bytes(z[4..8].try_into().unwrap()) as usize;
    let mut offs=vec![0usize;nb+1]; let mut p=8;
    for i in 0..nb{ offs[i+1]=offs[i]+u32::from_le_bytes(z[p..p+4].try_into().unwrap()) as usize; p+=4; }
    let body=&z[p..];
    let mut outs:Vec<Vec<u8>>=vec![vec![];nb];
    let nt=nt.max(1);
    std::thread::scope(|sc|{
        let mut hs=vec![];
        let mut rest:&mut [Vec<u8>]=&mut outs;
        let per=(nb+nt-1)/nt;
        let mut idx=0;
        while !rest.is_empty(){
            let take=per.min(rest.len());
            let (a,b)=rest.split_at_mut(take); rest=b;
            let lo=idx; idx+=take;
            let offs=&offs; 
            hs.push(sc.spawn(move||{
                for (j,o) in a.iter_mut().enumerate(){
                    let i=lo+j; *o=decode(&body[offs[i]..offs[i+1]]);
                }
            }));
        }
        for h in hs{h.join().unwrap();}
    });
    outs.concat()
}
fn main(){
    let a:Vec<String>=env::args().collect();
    if a.len()<3{ eprintln!("usage: mothergod c|d|t <in> [out]"); exit(1); }
    let data=fs::read(&a[2]).unwrap();
    match a[1].as_str(){
        "c"=>{ fs::write(&a[3],encode(&data)).unwrap(); }
        "d"=>{ fs::write(&a[3],decode(&data)).unwrap(); }
        "cp"=>{ let nt:usize=a.get(4).map(|x|x.parse().unwrap()).unwrap_or(4);
                fs::write(&a[3],encode_par(&data,nt)).unwrap(); }
        "dp"=>{ let nt:usize=a.get(4).map(|x|x.parse().unwrap()).unwrap_or(4);
                fs::write(&a[3],decode_par(&data,nt)).unwrap(); }
        "tp"=>{ let nt:usize=a.get(3).map(|x|x.parse().unwrap()).unwrap_or(4);
                let t0=Instant::now(); let z=encode_par(&data,nt); let te=t0.elapsed().as_secs_f64();
                let t1=Instant::now(); let r=decode_par(&z,nt); let td=t1.elapsed().as_secs_f64();
                assert!(r==data,"PAR ROUND TRIP FAILED");
                println!("{} PAR({} thr) ok {} -> {} ({:.3} b/B) enc {:.1} MB/s dec {:.1} MB/s",
                  a[2],nt,data.len(),z.len(),(z.len() as f64*8.0)/(data.len() as f64),
                  data.len() as f64/te/1e6, data.len() as f64/td/1e6); }
        "t"=>{ let t0=Instant::now(); let z=encode(&data); let te=t0.elapsed().as_secs_f64();
               let t1=Instant::now(); let r=decode(&z); let td=t1.elapsed().as_secs_f64();
               assert!(r==data,"ROUND TRIP FAILED");
               println!("{} ok {} -> {} ({:.3} b/B) enc {:.1} MB/s dec {:.1} MB/s",
                 a[2],data.len(),z.len(),(z.len() as f64*8.0)/(data.len() as f64),
                 data.len() as f64/te/1e6, data.len() as f64/td/1e6); }
        _=>exit(1)
    }
}
