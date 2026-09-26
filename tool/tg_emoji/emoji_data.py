# Cherrygram `EmojiData.java` ni o'qiydi (data, dataColored, alias).
import re, struct, os, shutil, io, sys
from PIL import Image
SRC='cg/TMessagesProj/src/main'
java=open(SRC+'/java/org/telegram/messenger/EmojiData.java',encoding='utf-8').read()

def jstr(s):
    # Java literal -> python str (surrogates via \u escapes)
    s=re.sub(r'\\u([0-9a-fA-F]{4})', lambda m: chr(int(m.group(1),16)), s)
    return s.encode('utf-16','surrogatepass').decode('utf-16')

def arr2(name):
    i=java.index('public static final String[][] %s = {'%name)
    j=java.index('\n    };',i)
    body=java[i:j]
    parts=re.split(r'new String\[\]\{|\bnull\b', body)[1:]
    # keep order including nulls
    tokens=re.findall(r'new String\[\]\{(.*?)\}|\b(null)\b', body, re.S)
    res=[]
    for blk,nul in tokens:
        if nul: res.append(None)
        else: res.append([jstr(x) for x in re.findall(r'"((?:[^"\\]|\\.)*)"', blk)])
    return res

def arr1(name):
    i=java.index('public static final String[] %s = new String[]{'%name) if ('String[] %s = new String[]{'%name) in java else java.index('public static final String[] %s = {'%name)
    j=java.index('};',i)
    return [jstr(x) for x in re.findall(r'"((?:[^"\\]|\\.)*)"', java[i:j])]

data=arr2('data'); colored=arr2('dataColored')
assert len(data)==8, len(data)
colored=[c if c is not None else data[k] for k,c in enumerate(colored)]
alias_old=arr1('aliasOld'); alias_new=arr1('aliasNew'); assert len(alias_old)==len(alias_new)

