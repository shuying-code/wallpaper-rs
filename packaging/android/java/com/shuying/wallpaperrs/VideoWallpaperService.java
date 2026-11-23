package com.shuying.wallpaperrs;

import android.graphics.SurfaceTexture;
import android.media.MediaMetadataRetriever;
import android.media.MediaPlayer;
import android.opengl.EGL14;
import android.opengl.EGLConfig;
import android.opengl.EGLContext;
import android.opengl.EGLDisplay;
import android.opengl.EGLSurface;
import android.opengl.GLES11Ext;
import android.opengl.GLES20;
import android.opengl.Matrix;
import android.os.Handler;
import android.os.Looper;
import android.service.wallpaper.WallpaperService;
import android.util.Log;
import android.view.Choreographer;
import android.view.Surface;
import android.view.SurfaceHolder;

import java.io.File;
import java.nio.ByteBuffer;
import java.nio.ByteOrder;
import java.nio.FloatBuffer;

/**
 * Plays the user-selected video as a looping, muted live wallpaper.
 *
 * MediaPlayer alone stretches a video to the wallpaper surface or crops it.
 * The video is decoded into a SurfaceTexture here and drawn by OpenGL with an
 * aspect-fit viewport, so the complete frame stays visible on every screen
 * shape.
 */
public final class VideoWallpaperService extends WallpaperService {
    private static final String TAG = "WallpaperStudio";
    private static final String VIDEO_RELATIVE_PATH = "live-wallpaper/current-video";
    private static final long FILE_POLL_MS = 1_500L;

    @Override
    public Engine onCreateEngine() {
        return new VideoEngine();
    }

    private final class VideoEngine extends Engine {
        private final Handler handler = new Handler(Looper.getMainLooper());
        private final Runnable filePoll = new Runnable() {
            @Override
            public void run() {
                if (isVisible()) {
                    ensurePlaying();
                    handler.postDelayed(this, FILE_POLL_MS);
                }
            }
        };
        private VideoRenderer renderer;
        private File loadedVideo;
        private long loadedModified;

        @Override
        public void onVisibilityChanged(boolean visible) {
            if (visible) {
                ensurePlaying();
                if (renderer != null) {
                    renderer.setVisible(true);
                }
                handler.removeCallbacks(filePoll);
                handler.postDelayed(filePoll, FILE_POLL_MS);
            } else {
                handler.removeCallbacks(filePoll);
                if (renderer != null) {
                    renderer.setVisible(false);
                }
            }
        }

        @Override
        public void onSurfaceCreated(SurfaceHolder holder) {
            Log.i(TAG, "live wallpaper surface created");
            ensurePlaying();
        }

        @Override
        public void onSurfaceChanged(SurfaceHolder holder, int format, int width, int height) {
            Log.i(TAG, "live wallpaper surface changed: " + width + "x" + height);
            if (renderer != null) {
                renderer.setOutput(holder.getSurface(), width, height);
            }
            ensurePlaying();
        }

        @Override
        public void onSurfaceDestroyed(SurfaceHolder holder) {
            Log.i(TAG, "live wallpaper surface destroyed");
            releaseRenderer();
        }

        @Override
        public void onDestroy() {
            handler.removeCallbacksAndMessages(null);
            releaseRenderer();
        }

        private void ensurePlaying() {
            if (!isVisible()) {
                return;
            }
            Surface surface = getSurfaceHolder().getSurface();
            File video = new File(getFilesDir(), VIDEO_RELATIVE_PATH);
            if (surface == null || !surface.isValid() || !video.isFile()) {
                if (video.isFile()) {
                    Log.w(TAG, "live wallpaper waiting for a valid surface");
                } else {
                    Log.w(TAG, "live wallpaper video is missing: " + video);
                }
                return;
            }

            long modified = video.lastModified();
            if (renderer != null
                    && (loadedVideo == null
                    || !loadedVideo.equals(video)
                    || modified != loadedModified)) {
                Log.i(TAG, "live wallpaper video changed, reloading");
                releaseRenderer();
            }
            if (renderer == null) {
                android.graphics.Rect frame = getSurfaceHolder().getSurfaceFrame();
                int width = frame.width() > 0 ? frame.width() : 1;
                int height = frame.height() > 0 ? frame.height() : 1;
                Log.i(TAG, "preparing live wallpaper video: " + video.length() + " bytes");
                renderer = new VideoRenderer(video, surface, width, height);
                loadedVideo = video;
                loadedModified = modified;
                if (!renderer.isReady()) {
                    releaseRenderer();
                    return;
                }
            }
            renderer.ensurePlaying();
        }

        private void releaseRenderer() {
            loadedModified = 0;
            if (renderer != null) {
                renderer.release();
                renderer = null;
            }
        }
    }

    private static final class VideoRenderer implements SurfaceTexture.OnFrameAvailableListener {
        private static final String VERTEX_SHADER =
                "attribute vec4 aPosition;\n"
                        + "attribute vec4 aTexCoord;\n"
                        + "uniform mat4 uTexMatrix;\n"
                        + "varying vec2 vTexCoord;\n"
                        + "void main() {\n"
                        + "  gl_Position = aPosition;\n"
                        + "  vTexCoord = (uTexMatrix * aTexCoord).xy;\n"
                        + "}\n";
        private static final String FRAGMENT_SHADER =
                "#extension GL_OES_EGL_image_external : require\n"
                        + "precision mediump float;\n"
                        + "uniform samplerExternalOES uTexture;\n"
                        + "varying vec2 vTexCoord;\n"
                        + "void main() {\n"
                        + "  gl_FragColor = texture2D(uTexture, vTexCoord);\n"
                        + "}\n";
        private static final float[] POSITIONS = {
                -1f, -1f,
                1f, -1f,
                -1f, 1f,
                1f, 1f,
        };
        private static final float[] TEX_COORDS = {
                0f, 0f,
                1f, 0f,
                0f, 1f,
                1f, 1f,
        };

        private final File videoFile;
        private final Handler handler = new Handler(Looper.getMainLooper());
        private final Choreographer choreographer = Choreographer.getInstance();
        private final float[] surfaceTextureMatrix = new float[16];
        private final float[] rotationMatrix = new float[16];
        private final float[] textureMatrix = new float[16];
        private final Choreographer.FrameCallback frameCallback =
                new Choreographer.FrameCallback() {
                    @Override
                    public void doFrame(long frameTimeNanos) {
                        framePosted = false;
                        if (frameAvailable && visible && !released) {
                            drawFrame();
                        }
                    }
                };

        private Surface outputSurface;
        private int outputWidth;
        private int outputHeight;
        private EGLDisplay eglDisplay = EGL14.EGL_NO_DISPLAY;
        private EGLContext eglContext = EGL14.EGL_NO_CONTEXT;
        private EGLSurface eglSurface = EGL14.EGL_NO_SURFACE;
        private EGLConfig eglConfig;
        private SurfaceTexture surfaceTexture;
        private Surface videoSurface;
        private MediaPlayer player;
        private FloatBuffer positionBuffer;
        private FloatBuffer texCoordBuffer;
        private int textureId;
        private int program;
        private int positionHandle;
        private int texCoordHandle;
        private int textureMatrixHandle;
        private int textureHandle;
        private int videoWidth;
        private int videoHeight;
        private int videoRotation;
        private boolean prepared;
        private boolean glReady;
        private boolean frameAvailable;
        private boolean framePosted;
        private boolean visible = true;
        private boolean released;

        VideoRenderer(File videoFile, Surface surface, int width, int height) {
            this.videoFile = videoFile;
            setOutput(surface, width, height);
            readVideoMetadata();
            if (!initGl()) {
                return;
            }
            glReady = true;
            ensurePlaying();
        }

        boolean isReady() {
            return glReady && !released;
        }

        void setOutput(Surface surface, int width, int height) {
            outputSurface = surface;
            outputWidth = Math.max(1, width);
            outputHeight = Math.max(1, height);
            if (glReady && !released) {
                releaseEglSurface();
                createEglSurface();
            }
        }

        void setVisible(boolean nextVisible) {
            visible = nextVisible;
            if (released) {
                return;
            }
            if (visible) {
                ensurePlaying();
            } else {
                choreographer.removeFrameCallback(frameCallback);
                framePosted = false;
                if (player != null && player.isPlaying()) {
                    player.pause();
                }
            }
        }

        void ensurePlaying() {
            if (released || !glReady || !visible) {
                return;
            }
            if (player == null) {
                createPlayer();
                return;
            }
            if (prepared && !player.isPlaying()) {
                player.start();
                scheduleFrame();
            }
        }

        private void createPlayer() {
            if (surfaceTexture == null) {
                return;
            }
            try {
                player = new MediaPlayer();
                player.setDataSource(videoFile.getAbsolutePath());
                if (videoWidth > 0 && videoHeight > 0) {
                    surfaceTexture.setDefaultBufferSize(videoWidth, videoHeight);
                }
                player.setSurface(videoSurface);
                player.setLooping(true);
                player.setVolume(0f, 0f);
                player.setOnPreparedListener(mediaPlayer -> {
                    if (released || player != mediaPlayer) {
                        return;
                    }
                    if (videoWidth <= 0 || videoHeight <= 0) {
                        videoWidth = mediaPlayer.getVideoWidth();
                        videoHeight = mediaPlayer.getVideoHeight();
                        if (videoWidth > 0 && videoHeight > 0) {
                            surfaceTexture.setDefaultBufferSize(videoWidth, videoHeight);
                        }
                    }
                    prepared = true;
                    Log.i(TAG, "live wallpaper video prepared: "
                            + videoWidth + "x" + videoHeight
                            + " rotation=" + videoRotation);
                    if (visible) {
                        mediaPlayer.start();
                        scheduleFrame();
                    }
                });
                player.setOnErrorListener((mediaPlayer, what, extra) -> {
                    Log.e(TAG, "live wallpaper MediaPlayer error: " + what + "/" + extra);
                    releasePlayer();
                    return true;
                });
                player.prepareAsync();
            } catch (Exception error) {
                Log.e(TAG, "live wallpaper failed to play", error);
                releasePlayer();
            }
        }

        private void readVideoMetadata() {
            MediaMetadataRetriever retriever = new MediaMetadataRetriever();
            try {
                retriever.setDataSource(videoFile.getAbsolutePath());
                videoWidth = parseMetadataInt(retriever,
                        MediaMetadataRetriever.METADATA_KEY_VIDEO_WIDTH);
                videoHeight = parseMetadataInt(retriever,
                        MediaMetadataRetriever.METADATA_KEY_VIDEO_HEIGHT);
                videoRotation = parseMetadataInt(retriever,
                        MediaMetadataRetriever.METADATA_KEY_VIDEO_ROTATION);
            } catch (Exception error) {
                Log.w(TAG, "live wallpaper could not read video metadata", error);
            } finally {
                try {
                    retriever.release();
                } catch (Exception ignored) {
                }
            }
        }

        private int parseMetadataInt(MediaMetadataRetriever retriever, int key) {
            String value = retriever.extractMetadata(key);
            if (value == null || value.isEmpty()) {
                return 0;
            }
            try {
                return Integer.parseInt(value);
            } catch (NumberFormatException ignored) {
                return 0;
            }
        }

        private boolean initGl() {
            eglDisplay = EGL14.eglGetDisplay(EGL14.EGL_DEFAULT_DISPLAY);
            if (eglDisplay == EGL14.EGL_NO_DISPLAY) {
                Log.e(TAG, "live wallpaper EGL display unavailable");
                return false;
            }
            int[] version = new int[2];
            if (!EGL14.eglInitialize(eglDisplay, version, 0, version, 1)) {
                Log.e(TAG, "live wallpaper eglInitialize failed");
                return false;
            }
            int[] configAttributes = {
                    EGL14.EGL_RED_SIZE, 8,
                    EGL14.EGL_GREEN_SIZE, 8,
                    EGL14.EGL_BLUE_SIZE, 8,
                    EGL14.EGL_ALPHA_SIZE, 8,
                    EGL14.EGL_SURFACE_TYPE, EGL14.EGL_WINDOW_BIT,
                    EGL14.EGL_RENDERABLE_TYPE, EGL14.EGL_OPENGL_ES2_BIT,
                    EGL14.EGL_NONE,
            };
            EGLConfig[] configs = new EGLConfig[1];
            int[] configCount = new int[1];
            if (!EGL14.eglChooseConfig(eglDisplay, configAttributes, 0,
                    configs, 0, configs.length, configCount, 0)
                    || configCount[0] <= 0) {
                Log.e(TAG, "live wallpaper EGL config unavailable");
                return false;
            }
            eglConfig = configs[0];
            int[] contextAttributes = {
                    EGL14.EGL_CONTEXT_CLIENT_VERSION, 2,
                    EGL14.EGL_NONE,
            };
            eglContext = EGL14.eglCreateContext(eglDisplay, eglConfig,
                    EGL14.EGL_NO_CONTEXT, contextAttributes, 0);
            if (eglContext == EGL14.EGL_NO_CONTEXT) {
                Log.e(TAG, "live wallpaper EGL context unavailable");
                return false;
            }
            if (!createEglSurface()) {
                return false;
            }
            return initProgram();
        }

        private boolean createEglSurface() {
            if (eglDisplay == EGL14.EGL_NO_DISPLAY
                    || eglConfig == null
                    || outputSurface == null
                    || !outputSurface.isValid()) {
                return false;
            }
            int[] attributes = {EGL14.EGL_NONE};
            eglSurface = EGL14.eglCreateWindowSurface(eglDisplay, eglConfig,
                    outputSurface, attributes, 0);
            if (eglSurface == EGL14.EGL_NO_SURFACE) {
                Log.e(TAG, "live wallpaper EGL window surface unavailable");
                return false;
            }
            if (!EGL14.eglMakeCurrent(eglDisplay, eglSurface, eglSurface, eglContext)) {
                Log.e(TAG, "live wallpaper eglMakeCurrent failed");
                return false;
            }
            return true;
        }

        private boolean initProgram() {
            int vertexShader = compileShader(GLES20.GL_VERTEX_SHADER, VERTEX_SHADER);
            int fragmentShader = compileShader(GLES20.GL_FRAGMENT_SHADER, FRAGMENT_SHADER);
            if (vertexShader == 0 || fragmentShader == 0) {
                return false;
            }
            program = GLES20.glCreateProgram();
            GLES20.glAttachShader(program, vertexShader);
            GLES20.glAttachShader(program, fragmentShader);
            GLES20.glLinkProgram(program);
            int[] linkStatus = new int[1];
            GLES20.glGetProgramiv(program, GLES20.GL_LINK_STATUS, linkStatus, 0);
            GLES20.glDeleteShader(vertexShader);
            GLES20.glDeleteShader(fragmentShader);
            if (linkStatus[0] != GLES20.GL_TRUE) {
                Log.e(TAG, "live wallpaper shader link failed: "
                        + GLES20.glGetProgramInfoLog(program));
                return false;
            }
            positionHandle = GLES20.glGetAttribLocation(program, "aPosition");
            texCoordHandle = GLES20.glGetAttribLocation(program, "aTexCoord");
            textureMatrixHandle = GLES20.glGetUniformLocation(program, "uTexMatrix");
            textureHandle = GLES20.glGetUniformLocation(program, "uTexture");
            positionBuffer = floatBuffer(POSITIONS);
            texCoordBuffer = floatBuffer(TEX_COORDS);

            int[] textures = new int[1];
            GLES20.glGenTextures(1, textures, 0);
            textureId = textures[0];
            GLES20.glBindTexture(GLES11Ext.GL_TEXTURE_EXTERNAL_OES, textureId);
            GLES20.glTexParameteri(GLES11Ext.GL_TEXTURE_EXTERNAL_OES,
                    GLES20.GL_TEXTURE_MIN_FILTER, GLES20.GL_LINEAR);
            GLES20.glTexParameteri(GLES11Ext.GL_TEXTURE_EXTERNAL_OES,
                    GLES20.GL_TEXTURE_MAG_FILTER, GLES20.GL_LINEAR);
            GLES20.glTexParameteri(GLES11Ext.GL_TEXTURE_EXTERNAL_OES,
                    GLES20.GL_TEXTURE_WRAP_S, GLES20.GL_CLAMP_TO_EDGE);
            GLES20.glTexParameteri(GLES11Ext.GL_TEXTURE_EXTERNAL_OES,
                    GLES20.GL_TEXTURE_WRAP_T, GLES20.GL_CLAMP_TO_EDGE);
            surfaceTexture = new SurfaceTexture(textureId);
            surfaceTexture.setOnFrameAvailableListener(this, handler);
            if (videoWidth > 0 && videoHeight > 0) {
                surfaceTexture.setDefaultBufferSize(videoWidth, videoHeight);
            }
            videoSurface = new Surface(surfaceTexture);
            return true;
        }

        private int compileShader(int type, String source) {
            int shader = GLES20.glCreateShader(type);
            GLES20.glShaderSource(shader, source);
            GLES20.glCompileShader(shader);
            int[] compiled = new int[1];
            GLES20.glGetShaderiv(shader, GLES20.GL_COMPILE_STATUS, compiled, 0);
            if (compiled[0] != GLES20.GL_TRUE) {
                Log.e(TAG, "live wallpaper shader compile failed: "
                        + GLES20.glGetShaderInfoLog(shader));
                GLES20.glDeleteShader(shader);
                return 0;
            }
            return shader;
        }

        private FloatBuffer floatBuffer(float[] values) {
            FloatBuffer buffer = ByteBuffer
                    .allocateDirect(values.length * Float.BYTES)
                    .order(ByteOrder.nativeOrder())
                    .asFloatBuffer();
            buffer.put(values).position(0);
            return buffer;
        }

        @Override
        public void onFrameAvailable(SurfaceTexture surfaceTexture) {
            frameAvailable = true;
            scheduleFrame();
        }

        private void scheduleFrame() {
            if (framePosted || !visible || released || !glReady) {
                return;
            }
            framePosted = true;
            choreographer.postFrameCallback(frameCallback);
        }

        private void drawFrame() {
            if (surfaceTexture == null || player == null) {
                return;
            }
            if (eglDisplay == EGL14.EGL_NO_DISPLAY
                    || eglSurface == EGL14.EGL_NO_SURFACE) {
                return;
            }
            if (!EGL14.eglMakeCurrent(eglDisplay, eglSurface, eglSurface, eglContext)) {
                Log.w(TAG, "live wallpaper lost its EGL context");
                return;
            }
            try {
                surfaceTexture.updateTexImage();
                surfaceTexture.getTransformMatrix(surfaceTextureMatrix);
            } catch (Exception error) {
                Log.w(TAG, "live wallpaper frame unavailable", error);
                frameAvailable = false;
                return;
            }
            frameAvailable = false;

            int displayWidth = videoWidth;
            int displayHeight = videoHeight;
            if (displayWidth <= 0 || displayHeight <= 0) {
                displayWidth = player.getVideoWidth();
                displayHeight = player.getVideoHeight();
            }
            if (displayWidth <= 0 || displayHeight <= 0) {
                return;
            }
            if (videoRotation == 90 || videoRotation == 270) {
                int swap = displayWidth;
                displayWidth = displayHeight;
                displayHeight = swap;
            }

            float scale = Math.min(
                    outputWidth / (float) displayWidth,
                    outputHeight / (float) displayHeight);
            int drawWidth = Math.max(1, Math.round(displayWidth * scale));
            int drawHeight = Math.max(1, Math.round(displayHeight * scale));
            int left = (outputWidth - drawWidth) / 2;
            int bottom = (outputHeight - drawHeight) / 2;

            if (videoRotation == 0) {
                System.arraycopy(surfaceTextureMatrix, 0, textureMatrix, 0, 16);
            } else {
                Matrix.setIdentityM(rotationMatrix, 0);
                Matrix.translateM(rotationMatrix, 0, 0.5f, 0.5f, 0f);
                Matrix.rotateM(rotationMatrix, 0, videoRotation, 0f, 0f, 1f);
                Matrix.translateM(rotationMatrix, 0, -0.5f, -0.5f, 0f);
                Matrix.multiplyMM(textureMatrix, 0, surfaceTextureMatrix, 0,
                        rotationMatrix, 0);
            }

            GLES20.glViewport(0, 0, outputWidth, outputHeight);
            GLES20.glClearColor(0f, 0f, 0f, 1f);
            GLES20.glClear(GLES20.GL_COLOR_BUFFER_BIT);
            GLES20.glViewport(left, bottom, drawWidth, drawHeight);
            GLES20.glUseProgram(program);
            GLES20.glActiveTexture(GLES20.GL_TEXTURE0);
            GLES20.glBindTexture(GLES11Ext.GL_TEXTURE_EXTERNAL_OES, textureId);
            GLES20.glUniform1i(textureHandle, 0);
            GLES20.glUniformMatrix4fv(textureMatrixHandle, 1, false,
                    textureMatrix, 0);
            GLES20.glEnableVertexAttribArray(positionHandle);
            GLES20.glVertexAttribPointer(positionHandle, 2, GLES20.GL_FLOAT,
                    false, 0, positionBuffer);
            GLES20.glEnableVertexAttribArray(texCoordHandle);
            GLES20.glVertexAttribPointer(texCoordHandle, 2, GLES20.GL_FLOAT,
                    false, 0, texCoordBuffer);
            GLES20.glDrawArrays(GLES20.GL_TRIANGLE_STRIP, 0, 4);
            GLES20.glDisableVertexAttribArray(positionHandle);
            GLES20.glDisableVertexAttribArray(texCoordHandle);
            if (!EGL14.eglSwapBuffers(eglDisplay, eglSurface)) {
                Log.w(TAG, "live wallpaper eglSwapBuffers failed");
            }
        }

        private void releasePlayer() {
            prepared = false;
            frameAvailable = false;
            choreographer.removeFrameCallback(frameCallback);
            framePosted = false;
            if (player != null) {
                try {
                    player.stop();
                } catch (Exception ignored) {
                }
                player.release();
                player = null;
            }
        }

        private void releaseEglSurface() {
            if (eglDisplay != EGL14.EGL_NO_DISPLAY
                    && eglSurface != EGL14.EGL_NO_SURFACE) {
                EGL14.eglMakeCurrent(eglDisplay, EGL14.EGL_NO_SURFACE,
                        EGL14.EGL_NO_SURFACE, EGL14.EGL_NO_CONTEXT);
                EGL14.eglDestroySurface(eglDisplay, eglSurface);
            }
            eglSurface = EGL14.EGL_NO_SURFACE;
        }

        void release() {
            if (released) {
                return;
            }
            released = true;
            visible = false;
            releasePlayer();
            if (program != 0 && eglDisplay != EGL14.EGL_NO_DISPLAY) {
                EGL14.eglMakeCurrent(eglDisplay, eglSurface, eglSurface, eglContext);
                GLES20.glDeleteProgram(program);
                program = 0;
            }
            if (textureId != 0 && eglDisplay != EGL14.EGL_NO_DISPLAY) {
                int[] textures = {textureId};
                GLES20.glDeleteTextures(1, textures, 0);
                textureId = 0;
            }
            if (surfaceTexture != null) {
                surfaceTexture.setOnFrameAvailableListener(null);
                surfaceTexture.release();
                surfaceTexture = null;
            }
            if (videoSurface != null) {
                videoSurface.release();
                videoSurface = null;
            }
            releaseEglSurface();
            if (eglDisplay != EGL14.EGL_NO_DISPLAY) {
                if (eglContext != EGL14.EGL_NO_CONTEXT) {
                    EGL14.eglDestroyContext(eglDisplay, eglContext);
                }
                EGL14.eglTerminate(eglDisplay);
            }
            eglContext = EGL14.EGL_NO_CONTEXT;
            eglDisplay = EGL14.EGL_NO_DISPLAY;
            eglConfig = null;
            glReady = false;
        }
    }
}
