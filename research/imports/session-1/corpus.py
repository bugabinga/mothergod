import os, math, glob, random, json, sqlite3, base64, tarfile, io, zlib

def _skewed_weights(target_H, n=256):
    def H(q):
        w=[q**i for i in range(n)]; s=sum(w)
        return -sum((x/s)*math.log2(x/s) for x in w if x)
    lo,hi=1e-6,0.999999
    for _ in range(60):
        mid=(lo+hi)/2
        if H(mid)<target_H: lo=mid
        else: hi=mid
    q=(lo+hi)/2
    return [q**i for i in range(n)]

def make_corpus(N=40000, seed=11):
    random.seed(seed); c={}; meta={}
    # ---------- real-world ----------
    docs=[]; seen=set()
    for p in sorted(glob.glob('/usr/share/doc/*/copyright'))[:200]:
        b=open(p,'rb').read()
        if b[:200] not in seen: seen.add(b[:200]); docs.append(b)
    c['text']=b'\n'.join(docs)[:N]
    c['code']=b''.join(open(p,'rb').read() for p in sorted(glob.glob('/usr/lib/python3*/[a-z]*.py'))[:60])[:N]
    import os as _os
    html=[p for p in sorted(glob.glob('/usr/share/doc/**/*.html',recursive=True)) if _os.path.isfile(p)]
    if html: c['html']=b''.join(open(p,'rb').read() for p in html[:30])[:N]
    loc=sorted(glob.glob('/usr/share/i18n/locales/*'))
    if loc: c['utf8-i18n']=b''.join(open(p,'rb').read() for p in loc[:8])[:N]
    pem=sorted(glob.glob('/etc/ssl/certs/*.pem'))
    if pem: c['pem-certs']=b''.join(open(p,'rb').read() for p in pem[:60])[:N]
    c['elf']=open('/usr/lib/x86_64-linux-gnu/libc.so.6','rb').read()[200000:200000+N]
    buf=io.BytesIO()
    with tarfile.open(fileobj=buf,mode='w') as t:
        for p in sorted(glob.glob('/usr/lib/python3*/[a-c]*.py'))[:25]: t.add(p)
    c['tar']=buf.getvalue()[:N]
    ips=[f'{random.randrange(1,255)}.{random.randrange(255)}.{random.randrange(255)}.{random.randrange(255)}' for _ in range(80)]
    paths=['/index.html','/api/v2/users','/api/v2/orders','/static/app.js','/favicon.ico','/login']
    c['log']='\n'.join(f'{random.choice(ips)} - - [19/Aug/2026:10:{i//60%60:02d}:{i%60:02d} +0200] "GET {random.choice(paths)} HTTP/1.1" {random.choice([200,200,200,304,404])} {random.randrange(200,50000)}' for i in range(1400)).encode()[:N]
    resp={'status':'ok','results':[{'user_id':1000+i,'name':f'user_{i}','email':f'user_{i}@example.com','active':random.random()<.8,'score':round(random.gauss(50,15),1)} for i in range(500)]}
    c['json']=json.dumps(resp).encode()[:N]
    db='/tmp/c.db'
    if os.path.exists(db): os.remove(db)
    con=sqlite3.connect(db)
    con.execute('create table m(ts int, s text, v real)')
    con.executemany('insert into m values(?,?,?)',[(1700000000+i*60,random.choice(['temp','hum','pres']),round(random.gauss(20,3),2)) for i in range(2500)])
    con.commit(); con.close()
    c['sqlite']=open(db,'rb').read()[:N]
    aud=[]
    for i in range(N//2):
        v=int(2500*math.sin(i/37)+1500*math.sin(i/11)+200*random.gauss(0,1))&0xffff
        aud+=[v&255,v>>8]
    c['audio16']=bytes(aud)
    img=[]
    for y in range(N//200+1):
        for x in range(200): img.append(int(90+70*math.sin(x/31)+50*math.sin(y/23)+8*random.gauss(0,1))&255)
    c['image']=bytes(img[:N])
    c['b64-text']=base64.b64encode(c['text'])[:N]
    c['zipped']=zlib.compress(c['text'],9)[:N]
    # ---------- entropy ladder (iid, exact targets) ----------
    for tH in (1.0,2.0,4.0,6.0):
        w=_skewed_weights(tH)
        c[f'iid-H{tH:.0f}']=bytes(random.choices(range(256),weights=w,k=N))
        meta[f'iid-H{tH:.0f}']=tH
    c['iid-H8']=os.urandom(N); meta['iid-H8']=8.0
    c['b64-random']=base64.b64encode(os.urandom(N))[:N]; meta['b64-random']=6.0
    # ---------- the trap: uniform histogram, low conditional entropy ----------
    w=_skewed_weights(2.0); deltas=random.choices(range(256),weights=w,k=N)
    out=[random.randrange(256)]
    for d in deltas[1:]: out.append((out[-1]+d)%256)
    c['markov-H8/2']=bytes(out); meta['markov-H8/2']=2.0
    return c, meta
