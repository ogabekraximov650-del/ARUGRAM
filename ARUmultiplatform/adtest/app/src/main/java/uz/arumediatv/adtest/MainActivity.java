package uz.arumediatv.adtest;

import android.app.Activity;
import android.os.Bundle;
import android.util.Log;

import com.google.android.gms.ads.AdRequest;
import com.google.android.gms.ads.LoadAdError;
import com.google.android.gms.ads.MobileAds;
import com.google.android.gms.ads.rewarded.RewardedAd;
import com.google.android.gms.ads.rewarded.RewardedAdLoadCallback;

/**
 * UZ: ikkita mukofotli reklamani ketma-ket yuklab ko'radi va natijani logcat'ga (teg `ADTEST`) yozadi:
 *  1) "control" — Google'ning SINOV reklama bloki (har doim yuklanishi kerak: SDK va tarmoq sog'lom ekanini isbotlaydi);
 *  2) "real" — bizning haqiqiy reklama blokimiz (xato kodi sababni ko'rsatadi: 3 = No fill, 1/8 = noto'g'ri ID).
 * EN: loads a control (Google test) rewarded unit and our real unit, logging results under tag ADTEST.
 * RU: gruzit kontrolnyy (testovyy) i nash realnyy rewarded-blok, rezultat v logcat s tegom ADTEST.
 */
public class MainActivity extends Activity {
    private static final String TAG = "ADTEST";
    private static final String CONTROL_UNIT = "ca-app-pub-3940256099942544/5224354917";
    private static final String REAL_UNIT = "ca-app-pub-8901205768474122/4115938931";

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
        Log.i(TAG, "ADTEST_START testDevice=" + new AdRequest.Builder().build().isTestDevice(this));
        MobileAds.initialize(this, status -> {
            Log.i(TAG, "ADTEST_INIT " + status.getAdapterStatusMap());
            load(CONTROL_UNIT, "control", () -> load(REAL_UNIT, "real", () -> Log.i(TAG, "ADTEST_DONE")));
        });
    }

    private void load(String unit, String label, Runnable next) {
        Log.i(TAG, "ADTEST_LOADING " + label + " unit=" + unit);
        RewardedAd.load(this, unit, new AdRequest.Builder().build(), new RewardedAdLoadCallback() {
            @Override
            public void onAdLoaded(RewardedAd ad) {
                Log.i(TAG, "ADTEST_RESULT " + label + " LOADED");
                next.run();
            }

            @Override
            public void onAdFailedToLoad(LoadAdError e) {
                Log.i(TAG, "ADTEST_RESULT " + label + " FAILED code=" + e.getCode() + " domain=" + e.getDomain() + " message=" + e.getMessage());
                next.run();
            }
        });
    }
}
