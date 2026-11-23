package com.shuying.wallpaperrs;

import android.content.Context;
import android.content.SharedPreferences;

/**
 * Shared storage for the boot-autostart switch. The native UI writes it through
 * JNI and BootReceiver reads it after the device finishes booting.
 */
public final class AutostartSettings {
    private static final String PREFS = "wallpaper_rs_settings";
    private static final String KEY_BOOT_AUTOSTART = "boot_autostart";

    private AutostartSettings() {
    }

    public static void setEnabled(Context context, boolean enabled) {
        preferences(context)
                .edit()
                .putBoolean(KEY_BOOT_AUTOSTART, enabled)
                .commit();
    }

    public static boolean isEnabled(Context context) {
        return preferences(context).getBoolean(KEY_BOOT_AUTOSTART, false);
    }

    private static SharedPreferences preferences(Context context) {
        return context.getSharedPreferences(PREFS, Context.MODE_PRIVATE);
    }
}
