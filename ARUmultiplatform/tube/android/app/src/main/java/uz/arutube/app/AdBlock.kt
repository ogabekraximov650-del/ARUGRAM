package uz.arutube.app

import android.net.Uri
import android.webkit.WebResourceResponse
import java.io.ByteArrayInputStream

/**
 * Reklama bloklovchi (WebView ichidagi pleyer sahifalari uchun).
 * 1) Ma'lum reklama/popunder tarmoqlariga so'rovlar bo'sh javob bilan almashtiriladi;
 * 2) `SCRIPT` sahifa (va iframe) yuklanishidan oldin qo'shiladi: `window.open` o'chiriladi, reklama elementlari va butun ekranni
 *    qoplovchi shaffof "bosing" qatlamlari olib tashlanadi.
 * Ro'yxat to'liq EMAS: DoodStream reklama domenlarini o'zgartirsa, yangilash kerak bo'ladi. Pleyer buzilsa — "Reklama bloki" tugmasi bilan o'chiriladi.
 */
object AdBlock {
    private val hosts = listOf(
        "doubleclick.net", "googlesyndication.com", "googleadservices.com", "adservice.google.com", "exoclick.com", "exosrv.com",
        "exdynsrv.com", "juicyads.com", "popads.net", "popcash.net", "propellerads.com", "adsterra.com", "clickadu.com",
        "hilltopads.net", "trafficjunky.com", "trafficstars.com", "a-ads.com", "adcash.com", "onclickads.net", "ad-maven.com",
        "admaven.com", "richpush.co", "bidgear.com", "mgid.com", "taboola.com", "outbrain.com", "revcontent.com", "adskeeper.com",
        "monetag.com", "pushance.com", "onclasrv.com", "histats.com", "popunderjs.com", "adnxs.com", "criteo.com", "yandex.ru",
    )
    private val pattern = Regex("""(^|[.\-])(adserver|adsystem|popunder|popads|clickunder|adsterra|exoclick)([.\-]|$)""", RegexOption.IGNORE_CASE)

    /** Pleyerning o'zi ishlashi uchun kerak bo'lgan Cloudflare tekshiruvi hech qachon bloklanmaydi. */
    fun isAd(u: Uri): Boolean {
        val h = u.host?.lowercase() ?: return false
        if (h.endsWith("cloudflare.com") || h.endsWith("cloudflareinsights.com")) return false
        return hosts.any { h == it || h.endsWith(".$it") } || pattern.containsMatchIn(h)
    }

    fun empty(): WebResourceResponse = WebResourceResponse("text/plain", "utf-8", ByteArrayInputStream(ByteArray(0)))

    const val SCRIPT = """(function(){
  try{window.open=function(){return null};}catch(e){}
  var bad=/doubleclick|googlesyndication|popunder|popads|exoclick|juicyads|adsterra|propeller|clickadu|hilltop|adcash/i;
  function overlay(e){
    var s=getComputedStyle(e);
    if(s.position!=='fixed'&&s.position!=='absolute')return false;
    var r=e.getBoundingClientRect();
    if(r.width<innerWidth*0.8||r.height<innerHeight*0.8)return false;
    if(e.tagName==='HTML'||e.tagName==='BODY'||e.tagName==='VIDEO')return false;
    if(e.querySelector('video,canvas,iframe,button,input'))return false;
    var cls=(e.className&&e.className.toString?e.className.toString():'')+' '+(e.id||'');
    if(/player|video|jw|vjs|control|dood/i.test(cls))return false;
    return (e.innerText||'').trim().length<40;
  }
  function clean(){
    document.querySelectorAll('iframe,a,div,ins').forEach(function(e){
      var src=(e.src||e.href||'')+' '+(e.className&&e.className.toString?e.className.toString():'')+' '+(e.id||'');
      if(bad.test(src)){e.remove();return;}
      if(e.tagName!=='IFRAME'&&overlay(e)){e.remove();}
    });
  }
  var n=0;var iv=setInterval(function(){try{clean()}catch(e){}if(++n>40)clearInterval(iv);},700);
  document.addEventListener('DOMContentLoaded',function(){try{clean()}catch(e){}});
})();"""
}
