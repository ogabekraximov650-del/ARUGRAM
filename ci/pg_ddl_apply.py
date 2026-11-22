import glob,re,json,urllib.request,sys
K=open('/tmp/rpc_k.txt').read().strip()
URL='https://rlakikrsudbfveajyggv.supabase.co/rest/v1/rpc/aru_pipeline'
ANON='sb_publishable_c1ajXglSd3JsdcDwTbjXWw_CSJDhOZ6'
kw=re.compile(r'^\s*(create\s+table|alter\s+table\s+\w+\s+add\s+column|create\s+(unique\s+)?index)',re.I)
out=[]
for f in sorted(glob.glob('/home/user/ARUGRAM/worker/src/*.rs')):
    s=open(f,encoding='utf8').read()
    for m in re.finditer(r'"((?:[^"\\]|\\.)*)"',s,re.S):
        t=m.group(1).replace("\\\n","").replace("\\n","\n").replace('\\"','"')
        t=re.sub(r'\s+',' ',t).strip()
        if kw.match(t) and '{' not in t and t not in out: out.append(t)
print(len(out),'ta DDL')
reqs=[{"type":"execute","stmt":{"sql":q,"args":[]}} for q in out]
body=json.dumps({"k":K,"req":{"requests":reqs}}).encode()
r=urllib.request.Request(URL,body,{'apikey':ANON,'Authorization':'Bearer '+ANON,'Content-Type':'application/json'})
res=json.load(urllib.request.urlopen(r,timeout=120))
for q,x in zip(out,res['results']):
    if x['type']=='error': print('XATO',x['error']['message'][:120],'|',q[:100])
print('tugadi')
