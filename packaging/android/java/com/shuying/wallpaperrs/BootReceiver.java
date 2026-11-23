package com.shuying.wallpaperrs;

import android.app.Notification;
import android.app.NotificationChannel;
import android.app.NotificationManager;
import android.app.PendingIntent;
import android.content.BroadcastReceiver;
import android.content.Context;
import android.content.Intent;
import android.os.Build;
import android.util.Log;

/**
 * Starts the wallpaper app after boot when the user enabled the option.
 *
 * Android 10+ and several phone vendors may reject a background Activity
 * launch. In that case we leave a tappable notification instead of failing
 * silently.
 */
public final class BootReceiver extends BroadcastReceiver {
    private static final String TAG = "WallpaperStudio";
    private static final String CHANNEL_ID = "boot_autostart";
    private static final int NOTIFICATION_ID = 0x5751;

    @Override
    public void onReceive(Context context, Intent intent) {
        String action = intent == null ? null : intent.getAction();
        if (!Intent.ACTION_BOOT_COMPLETED.equals(action)
                && !"android.intent.action.QUICKBOOT_POWERON".equals(action)
                && !"com.htc.intent.action.QUICKBOOT_POWERON".equals(action)) {
            return;
        }
        if (!AutostartSettings.isEnabled(context)) {
            return;
        }

        Intent launch = context.getPackageManager()
                .getLaunchIntentForPackage(context.getPackageName());
        if (launch != null) {
            // Android 10+ may silently ignore a background Activity launch.
            // Post the tappable fallback first; the app cancels it on startup.
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
                postOpenNotification(context);
            }
            launch.addFlags(
                    Intent.FLAG_ACTIVITY_NEW_TASK
                            | Intent.FLAG_ACTIVITY_CLEAR_TOP
                            | Intent.FLAG_ACTIVITY_SINGLE_TOP);
            try {
                context.startActivity(launch);
                Log.i(TAG, "boot autostart: activity launch requested");
                return;
            } catch (RuntimeException error) {
                Log.w(TAG, "boot autostart: background activity launch rejected", error);
            }
        }

        postOpenNotification(context);
    }

    /** Remove the boot fallback after the application window has started. */
    public static void clearBootNotification(Context context) {
        NotificationManager manager =
                (NotificationManager) context.getSystemService(Context.NOTIFICATION_SERVICE);
        if (manager != null) {
            manager.cancel(NOTIFICATION_ID);
        }
    }

    private void postOpenNotification(Context context) {
        NotificationManager manager =
                (NotificationManager) context.getSystemService(Context.NOTIFICATION_SERVICE);
        if (manager == null) {
            return;
        }
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            NotificationChannel channel = new NotificationChannel(
                    CHANNEL_ID,
                    "开机启动",
                    NotificationManager.IMPORTANCE_DEFAULT);
            manager.createNotificationChannel(channel);
        }

        Intent launch = context.getPackageManager()
                .getLaunchIntentForPackage(context.getPackageName());
        if (launch == null) {
            return;
        }
        launch.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK | Intent.FLAG_ACTIVITY_CLEAR_TOP);
        PendingIntent contentIntent = PendingIntent.getActivity(
                context,
                0,
                launch,
                PendingIntent.FLAG_UPDATE_CURRENT | PendingIntent.FLAG_IMMUTABLE);

        Notification.Builder builder = Build.VERSION.SDK_INT >= Build.VERSION_CODES.O
                ? new Notification.Builder(context, CHANNEL_ID)
                : new Notification.Builder(context);
        builder.setSmallIcon(android.R.drawable.ic_menu_gallery)
                .setContentTitle("Wallpaper Studio")
                .setContentText("开机自启已开启，点击打开应用")
                .setContentIntent(contentIntent)
                .setAutoCancel(true)
                .setOnlyAlertOnce(true);
        try {
            manager.notify(NOTIFICATION_ID, builder.build());
        } catch (RuntimeException error) {
            Log.w(TAG, "boot autostart: notification unavailable", error);
        }
    }
}
