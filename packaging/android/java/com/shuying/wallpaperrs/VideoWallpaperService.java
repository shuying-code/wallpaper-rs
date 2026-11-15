package com.shuying.wallpaperrs;

import android.media.MediaPlayer;
import android.service.wallpaper.WallpaperService;
import android.view.Surface;
import android.view.SurfaceHolder;

import java.io.File;

/**
 * Plays the user-selected video (files/live-wallpaper/current-video, written by
 * the Rust side before launching the system "change live wallpaper" dialog) as
 * a looping, muted live wallpaper.
 */
public final class VideoWallpaperService extends WallpaperService {
    private static final String VIDEO_RELATIVE_PATH = "live-wallpaper/current-video";

    @Override
    public Engine onCreateEngine() {
        return new VideoEngine();
    }

    private final class VideoEngine extends Engine {
        private MediaPlayer player;
        private long loadedModified;

        @Override
        public void onVisibilityChanged(boolean visible) {
            if (visible) {
                start();
            } else {
                pause();
            }
        }

        @Override
        public void onSurfaceChanged(SurfaceHolder holder, int format, int width, int height) {
            if (isVisible()) {
                start();
            }
        }

        @Override
        public void onSurfaceDestroyed(SurfaceHolder holder) {
            release();
        }

        @Override
        public void onDestroy() {
            release();
        }

        private void start() {
            Surface surface = getSurfaceHolder().getSurface();
            File video = new File(getFilesDir(), VIDEO_RELATIVE_PATH);
            if (surface == null || !surface.isValid() || !video.isFile()) {
                return;
            }
            long modified = video.lastModified();
            try {
                // 用户换了视频：当前文件已被原子替换，重载新内容。
                if (player != null && modified != loadedModified) {
                    release();
                }
                if (player == null) {
                    loadedModified = modified;
                    player = new MediaPlayer();
                    player.setDataSource(video.getAbsolutePath());
                    player.setSurface(surface);
                    player.setLooping(true);
                    player.setVolume(0f, 0f);
                    player.setVideoScalingMode(
                            MediaPlayer.VIDEO_SCALING_MODE_SCALE_TO_FIT_WITH_CROPPING);
                    player.setOnPreparedListener(MediaPlayer::start);
                    player.setOnErrorListener((mediaPlayer, what, extra) -> {
                        release();
                        return true;
                    });
                    player.prepareAsync();
                } else if (!player.isPlaying()) {
                    player.start();
                }
            } catch (Exception error) {
                release();
            }
        }

        private void pause() {
            if (player != null && player.isPlaying()) {
                player.pause();
            }
        }

        private void release() {
            loadedModified = 0;
            if (player != null) {
                try {
                    player.stop();
                } catch (Exception ignored) {
                }
                player.release();
                player = null;
            }
        }
    }
}
