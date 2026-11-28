// Adapted from ysp-live (assets/auth_bridge.js), MIT licensed.
// Copyright (c) 2026 ysp-live contributors.
(function () {
  if (window.__yspBridgeInstalled) return;
  window.__yspBridgeInstalled = true;

  var lastUrl = "";
  var lastPid = "";
  var lastDefn = "";
  var captured = false;
  var nativeOwnsStream = false;
  var nudgeTimer = null;
  var nudgeAttempts = 0;
  var focusedVideo = null;
  var requestedPid = "";
  var lastVideoWidth = 0;
  var lastVideoHeight = 0;
  var nextVideoId = 0;
  var backupVideo = null;
  var backupHls = null;
  var lastHeartbeatTime = -1;
  var heartbeatTimer = null;

  function requestedPidFromLocation() {
    try {
      var match = /[?&]pid=([^&#]+)/.exec(location.href);
      return match ? decodeURIComponent(match[1]) : "";
    } catch (_) {
      return "";
    }
  }

  requestedPid = requestedPidFromLocation();

  function syncNativeMode() {
    try {
      if (
        window.YspAndroidBridge &&
        typeof window.YspAndroidBridge.isOfficialPlayerMode === "function"
      ) {
        nativeOwnsStream = !window.YspAndroidBridge.isOfficialPlayerMode();
        if (nativeOwnsStream) silenceOfficialPlayback();
      }
    } catch (_) {}
  }

  function allowedHost() {
    var host = String(location.hostname || "").toLowerCase();
    return (
      host === "yangshipin.cn" ||
      host.endsWith(".yangshipin.cn") ||
      host === "cctv.com" ||
      host.endsWith(".cctv.com") ||
      host === "cctv.cn" ||
      host.endsWith(".cctv.cn") ||
      host === "cntv.cn" ||
      host.endsWith(".cntv.cn")
    );
  }

  function post(payload) {
    if (!allowedHost()) return;
    var text;
    try {
      text = JSON.stringify(payload);
    } catch (_) {
      return;
    }

    try {
      if (window.ipc && typeof window.ipc.postMessage === "function") {
        window.ipc.postMessage(text);
      } else if (
        window.YspAndroidBridge &&
        typeof window.YspAndroidBridge.postMessage === "function"
      ) {
        window.YspAndroidBridge.postMessage(text);
      } else if (
        window.chrome &&
        window.chrome.webview &&
        typeof window.chrome.webview.postMessage === "function"
      ) {
        window.chrome.webview.postMessage(text);
      }
    } catch (_) {
      // The host may have destroyed the bridge while the page was unloading.
    }
  }

  function simplifyFormats(formats) {
    if (!Array.isArray(formats)) return [];
    return formats.map(function (item) {
      return {
        id: item && item.id,
        fn: item && item.fn,
        fnname: item && item.fnname,
        defnname: item && item.defnname,
        defnrate: item && item.defnrate,
        encrypt: item && item.encrypt,
      };
    });
  }

  function findOfficialVideo() {
    if (
      window.__yspBackupActive &&
      backupVideo &&
      backupVideo.isConnected
    ) {
      return backupVideo;
    }
    var videos = document.querySelectorAll("video");
    if (!videos.length) return null;
    var best = null;
    var bestArea = 0;
    for (var i = 0; i < videos.length; i += 1) {
      var candidate = videos[i];
      if (!candidate || !candidate.isConnected) continue;
      var rect = candidate.getBoundingClientRect();
      var area = Math.max(0, rect.width) * Math.max(0, rect.height);
      if (!best || area > bestArea) {
        best = candidate;
        bestArea = area;
      }
    }
    return best || videos[0];
  }

  function videoIdentity(video) {
    if (!video) return "";
    if (!video.__yspHostVideoId) {
      nextVideoId += 1;
      video.__yspHostVideoId = "v" + nextVideoId;
    }
    return video.__yspHostVideoId;
  }

  function isBackupMedia(video) {
    return Boolean(
      window.__yspBackupActive && backupVideo && video === backupVideo
    );
  }

  function isStaleOfficialMedia(video) {
    return Boolean(
      window.__yspBackupActive && backupVideo && video !== backupVideo
    );
  }

  function videoDimensions(video) {
    if (!video) return null;
    var width = Number(video.videoWidth || 0);
    var height = Number(video.videoHeight || 0);
    if (!isFinite(width) || !isFinite(height) || width <= 0 || height <= 0) {
      return null;
    }
    return {
      width: Math.round(width),
      height: Math.round(height),
    };
  }

  function reportVideoSize(video, force) {
    if (nativeOwnsStream || !video || isStaleOfficialMedia(video)) return null;
    var dimensions = videoDimensions(video);
    if (!dimensions) return null;
    var width = dimensions.width;
    var height = dimensions.height;
    if (
      !force &&
      width === lastVideoWidth &&
      height === lastVideoHeight
    ) {
      return dimensions;
    }
    lastVideoWidth = width;
    lastVideoHeight = height;
    post({
      type: "video_size",
      width: width,
      height: height,
      backup: isBackupMedia(video),
    });
    return dimensions;
  }

  function startVideoHeartbeat() {
    if (heartbeatTimer) return;
    heartbeatTimer = window.setInterval(function () {
      if (nativeOwnsStream) {
        silenceOfficialPlayback();
        return;
      }
      var video = findOfficialVideo();
      if (!video || video.paused || Number(video.readyState || 0) < 2) {
        return;
      }
      var dimensions = videoDimensions(video);
      var currentTime = Number(video.currentTime || 0);
      if (!dimensions || !isFinite(currentTime)) {
        return;
      }
      if (currentTime < lastHeartbeatTime - 1) {
        lastHeartbeatTime = -1;
      }
      if (currentTime <= lastHeartbeatTime + 0.04) {
        return;
      }
      lastHeartbeatTime = currentTime;
      post({
        type: "video_progress",
        currentTime: currentTime,
        width: dimensions.width,
        height: dimensions.height,
        backup: isBackupMedia(video),
      });
    }, 2000);
  }

  function focusOfficialVideo(force) {
    if (nativeOwnsStream) return false;
    var video = findOfficialVideo();
    if (!video) return false;
    if (!force && video === focusedVideo && video.isConnected) return true;
    focusedVideo = video;
    var node = video;

    while (node && node !== document.body && node.parentElement) {
      var parent = node.parentElement;
      var children = parent.children;
      for (var i = 0; i < children.length; i += 1) {
        var child = children[i];
        if (child === node) continue;
        if (/^(SCRIPT|STYLE|LINK|NOSCRIPT)$/.test(child.tagName)) continue;
        child.style.setProperty("display", "none", "important");
      }

      node.style.setProperty("position", "fixed", "important");
      node.style.setProperty("inset", "0", "important");
      node.style.setProperty("width", "100vw", "important");
      node.style.setProperty("height", "100vh", "important");
      node.style.setProperty("transform", "none", "important");
      node.style.setProperty("overflow", "hidden", "important");
      node.style.setProperty("background", "#000", "important");
      node.style.setProperty("z-index", "2147483647", "important");
      node = parent;
    }

    document.documentElement.style.setProperty("background", "#000", "important");
    document.body.style.setProperty("margin", "0", "important");
    document.body.style.setProperty("overflow", "hidden", "important");
    document.body.style.setProperty("background", "#000", "important");
    video.style.setProperty("width", "100vw", "important");
    video.style.setProperty("height", "100vh", "important");
    video.style.setProperty("object-fit", "contain", "important");
    video.style.setProperty("background", "#000", "important");
    video.controls = false;
    reportVideoSize(video);
    return true;
  }

  window.__yspFocusOfficialVideo = function () {
    return focusOfficialVideo(true);
  };

  function handlePlayerResponse(data) {
    if (!data || typeof data !== "object") return;
    var body = data.data && typeof data.data === "object" ? data.data : data;
    var playurl = body && body.playurl;
    if (typeof playurl !== "string" || !playurl) return;
    var definition = body.defn == null ? "" : String(body.defn);
    var livePid =
      body.livepid == null ? "" : String(body.livepid);
    if (requestedPid && livePid && livePid !== requestedPid) return;
    if (
      playurl === lastUrl &&
      livePid === lastPid &&
      definition === lastDefn
    ) {
      return;
    }
    lastUrl = playurl;
    lastPid = livePid;
    lastDefn = definition;
    captured = true;
    stopNudge();

    post({
      type: "live_info",
      playurl: playurl,
      defn: body.defn || null,
      livepid: livePid || null,
      encrypt: body.encrypt == null ? null : body.encrypt,
      formats: simplifyFormats(body.formats),
      backupurls: backupUrlsFrom(body),
    });

  }

  function backupUrlsFrom(body) {
    var list = body && body.backurl_list;
    if (!Array.isArray(list)) return [];
    var result = [];
    for (var index = 0; index < list.length; index += 1) {
      var item = list[index];
      var candidate =
        typeof item === "string"
          ? item
          : item && typeof item.url === "string"
            ? item.url
            : "";
      if (!candidate) continue;
      try {
        var parsed = new URL(candidate, location.href);
        if (
          parsed.protocol !== "https:" ||
          !parsed.pathname.toLowerCase().endsWith(".m3u8")
        ) {
          continue;
        }
        if (result.indexOf(parsed.href) < 0) result.push(parsed.href);
      } catch (_) {}
      if (result.length >= 4) break;
    }
    return result;
  }

  function parseMaybeJson(text) {
    if (typeof text !== "string") return null;
    try {
      return JSON.parse(text);
    } catch (_) {
      return null;
    }
  }

  function isLiveInfoUrl(url) {
    try {
      var parsed = new URL(String(url), location.href);
      return /\/v1\/player\/get_live_info\/?$/.test(parsed.pathname);
    } catch (_) {
      return String(url || "").indexOf("/v1/player/get_live_info") >= 0;
    }
  }

  var originalOpen = XMLHttpRequest.prototype.open;
  XMLHttpRequest.prototype.open = function (method, url) {
    this.__yspRequestUrl = String(url || "");
    return originalOpen.apply(this, arguments);
  };

  var originalSend = XMLHttpRequest.prototype.send;
  XMLHttpRequest.prototype.send = function () {
    var xhr = this;
    if (isLiveInfoUrl(xhr.__yspRequestUrl)) {
      xhr.addEventListener("load", function () {
        try {
          if (typeof xhr.responseText !== "string") return;
          var data = parseMaybeJson(xhr.responseText);
          if (data) handlePlayerResponse(data);
        } catch (_) {}
      });
    }
    return originalSend.apply(this, arguments);
  };

  if (typeof window.fetch === "function") {
    var originalFetch = window.fetch;
    window.fetch = function (input, init) {
      var url =
        typeof input === "string"
          ? input
          : input && input.url
            ? input.url
            : "";
      var promise = originalFetch.apply(this, arguments);
      if (!isLiveInfoUrl(url)) return promise;

      return promise.then(function (response) {
        try {
          response
            .clone()
            .json()
            .then(function (data) {
              handlePlayerResponse(data);
            })
            .catch(function () {});
        } catch (_) {}
        return response;
      });
    };
  }

  function nudgePlayback() {
    if (nativeOwnsStream) {
      silenceOfficialPlayback();
      return;
    }
    if (captured) {
      stopNudge();
      return;
    }
    nudgeAttempts += 1;
    if (nudgeAttempts > 8) {
      stopNudge();
      return;
    }
    focusOfficialVideo(false);
    var video = findOfficialVideo();
    if (!video) return;
    try {
      video.muted = false;
      video.volume = 1;
      if (video.paused) {
        var result = video.play();
        if (result && typeof result.catch === "function") result.catch(function () {});
      }
    } catch (_) {}
  }

  function stopNudge() {
    if (nudgeTimer) {
      clearInterval(nudgeTimer);
      nudgeTimer = null;
    }
  }

  function startNudge() {
    stopNudge();
    nudgeAttempts = 0;
    nudgeTimer = window.setInterval(nudgePlayback, 2000);
    window.setTimeout(nudgePlayback, 600);
  }

  startNudge();
  startVideoHeartbeat();
  syncNativeMode();

  document.addEventListener(
    "loadedmetadata",
    function (event) {
      var target = event && event.target;
      if (target && String(target.tagName || "").toUpperCase() === "VIDEO") {
        if (isStaleOfficialMedia(target)) return;
        reportVideoSize(target);
      }
    },
    true,
  );

  document.addEventListener(
    "playing",
    function (event) {
      var target = event && event.target;
      if (
        target &&
        String(target.tagName || "").toUpperCase() === "VIDEO"
      ) {
        if (window.__yspBackupActive && target !== backupVideo) {
          try {
            target.muted = true;
            target.pause();
          } catch (_) {}
          return;
        }
        reportVideoSize(target);
      }
    },
    true,
  );

  window.addEventListener("resize", function () {
    reportVideoSize(findOfficialVideo());
  });

  document.addEventListener(
    "DOMContentLoaded",
    function () {
      if (!requestedPid) requestedPid = requestedPidFromLocation();
      syncNativeMode();
    },
    { once: true },
  );

  function silenceOfficialPlayback() {
    try {
      document.querySelectorAll("video,audio").forEach(function (video) {
        video.muted = true;
        if (!video.paused) video.pause();
      });
    } catch (_) {}
  }

  function stopBackupPlayback() {
    if (backupHls) {
      try {
        backupHls.destroy();
      } catch (_) {}
      backupHls = null;
    }
    if (backupVideo) {
      try {
        backupVideo.pause();
        backupVideo.removeAttribute("src");
        backupVideo.load();
        backupVideo.remove();
      } catch (_) {}
      backupVideo = null;
    }
    window.__yspBackupActive = false;
    lastHeartbeatTime = -1;
    lastVideoWidth = 0;
    lastVideoHeight = 0;
    var app = document.getElementById("app");
    if (app) {
      app.style.setProperty("display", "block", "important");
      window.dispatchEvent(new Event("resize"));
    }
  }

  window.__yspStopBackupPlayer = function () {
    stopBackupPlayback();
    return true;
  };

  window.__yspPlayBackupUrl = function (raw) {
    if (nativeOwnsStream) return false;
    var source = String(raw || "");
    try {
      var parsed = new URL(source, location.href);
      if (
        parsed.protocol !== "https:" ||
        !parsed.pathname.toLowerCase().endsWith(".m3u8")
      ) {
        return false;
      }
    } catch (_) {
      return false;
    }

    stopBackupPlayback();
    silenceOfficialPlayback();
    try {
      document.querySelectorAll("video,audio").forEach(function (candidate) {
        candidate.muted = true;
        candidate.pause();
      });
    } catch (_) {}

    var video = document.createElement("video");
    video.id = "yspBackupVideo";
    video.autoplay = true;
    video.playsInline = true;
    video.controls = false;
    video.muted = false;
    video.volume = 1;
    video.style.setProperty("position", "fixed", "important");
    video.style.setProperty("inset", "0", "important");
    video.style.setProperty("width", "100vw", "important");
    video.style.setProperty("height", "100vh", "important");
    video.style.setProperty("background", "#000", "important");
    video.style.setProperty("z-index", "2147483647", "important");
    video.style.setProperty("object-fit", "contain", "important");
    video.style.setProperty("display", "block", "important");
    video.addEventListener("error", function () {
      var code = video.error ? Number(video.error.code || 0) : 0;
      post({
        type: "playback_error",
        backup: true,
        message: "备用线路 video error " + code,
      });
    });
    document.body.appendChild(video);
    backupVideo = video;
    window.__yspBackupActive = true;
    document.documentElement.style.setProperty("background", "#000", "important");
    document.body.style.setProperty("margin", "0", "important");
    document.body.style.setProperty("overflow", "hidden", "important");
    document.body.style.setProperty("background", "#000", "important");

    if (window.Hls && window.Hls.isSupported()) {
      var hls = new window.Hls({
        enableWorker: true,
        lowLatencyMode: false,
        liveSyncDurationCount: 3,
        maxBufferLength: 30,
        backBufferLength: 30,
      });
      backupHls = hls;
      hls.on(window.Hls.Events.ERROR, function (_, data) {
        if (!data || !data.fatal) return;
        post({
          type: "playback_error",
          backup: true,
          message: "备用线路 HLS " + String(data.details || data.type || "错误"),
        });
      });
      hls.on(window.Hls.Events.MANIFEST_PARSED, function () {
        try {
          var playResult = video.play();
          if (playResult && typeof playResult.catch === "function") {
            playResult.catch(function () {});
          }
        } catch (_) {}
      });
      hls.attachMedia(video);
      hls.loadSource(source);
    } else {
      video.src = source;
      try {
        var playResult = video.play();
        if (playResult && typeof playResult.catch === "function") {
          playResult.catch(function () {});
        }
      } catch (_) {}
    }
    return true;
  };

  window.__yspPauseOfficialVideo = function () {
    silenceOfficialPlayback();
    return true;
  };

  window.__yspResumeOfficialVideo = function () {
    if (nativeOwnsStream) return false;
    var video = findOfficialVideo();
    if (!video) return false;
    try {
      document.querySelectorAll("video,audio").forEach(function (candidate) {
        if (candidate !== video) {
          candidate.muted = true;
          candidate.volume = 0;
        }
      });
      video.muted = false;
      video.volume = 1;
      if (video.paused) {
        var result = video.play();
        if (result && typeof result.catch === "function") result.catch(function () {});
      }
    } catch (_) {}
    focusOfficialVideo(true);
    return true;
  };

  window.__yspUseOfficialPlayer = function () {
    stopBackupPlayback();
    nativeOwnsStream = false;
    stopNudge();
    var video = findOfficialVideo();
    if (!video) return false;
    focusOfficialVideo(true);
    return window.__yspResumeOfficialVideo();
  };

  window.__yspRecoverOfficialVideo = function () {
    nativeOwnsStream = false;
    var video = findOfficialVideo();
    if (!video) return false;
    try {
      var ranges = video.seekable;
      if (ranges && ranges.length) {
        var end = Number(ranges.end(ranges.length - 1));
        var current = Number(video.currentTime || 0);
        if (isFinite(end) && end > 0 && end - current > 2) {
          video.currentTime = Math.max(0, end - 0.35);
        }
      }
      video.muted = false;
      video.volume = 1;
      if (video.paused) {
        var result = video.play();
        if (result && typeof result.catch === "function") result.catch(function () {});
      }
    } catch (_) {}
    focusOfficialVideo(true);
    return true;
  };

  window.__yspGetOfficialState = function () {
    var video = findOfficialVideo();
    if (!video) return "";
    var mediaError = video.error
      ? {
          code: Number(video.error.code || 0),
          message: String(video.error.message || ""),
        }
      : null;
    return JSON.stringify({
      videoId: videoIdentity(video),
      paused: Boolean(video.paused),
      readyState: Number(video.readyState || 0),
      networkState: Number(video.networkState || 0),
      currentTime: Number(video.currentTime || 0),
      videoWidth: Number(video.videoWidth || 0),
      videoHeight: Number(video.videoHeight || 0),
      error: mediaError,
    });
  };

  document.addEventListener(
    "play",
    function (event) {
      if (!nativeOwnsStream) return;
      var target = event.target;
      if (!target || !/^(VIDEO|AUDIO)$/.test(target.tagName)) return;
      try {
        target.muted = true;
        target.pause();
      } catch (_) {}
    },
    true,
  );

  document.addEventListener(
    "playing",
    function (event) {
      if (nativeOwnsStream) return;
      var target = event.target;
      if (!target || !/^(VIDEO|AUDIO)$/.test(target.tagName)) return;
      if (window.__yspBackupActive && target !== backupVideo) {
        try {
          target.muted = true;
          target.pause();
        } catch (_) {}
        return;
      }
      stopNudge();
      focusOfficialVideo(false);
      var dimensions = reportVideoSize(target, true);
      post({
        type: "playing",
        width: dimensions ? dimensions.width : 0,
        height: dimensions ? dimensions.height : 0,
        backup: isBackupMedia(target),
      });
    },
    true,
  );

  document.addEventListener(
    "waiting",
    function (event) {
      if (nativeOwnsStream) return;
      var target = event.target;
      if (!target || target.tagName !== "VIDEO") return;
      if (window.__yspBackupActive && target !== backupVideo) {
        try {
          target.muted = true;
          target.pause();
        } catch (_) {}
        return;
      }
      var dimensions = videoDimensions(target);
      post({
        type: "buffering",
        width: dimensions ? dimensions.width : 0,
        height: dimensions ? dimensions.height : 0,
        backup: isBackupMedia(target),
      });
    },
    true,
  );

  document.addEventListener(
    "error",
    function (event) {
      if (nativeOwnsStream) return;
      var target = event && event.target;
      if (!target || target.tagName !== "VIDEO") return;
      if (window.__yspBackupActive && target !== backupVideo) return;
      var code = target.error ? target.error.code : 0;
      post({
        type: "playback_error",
        backup: isBackupMedia(target),
        message: "video error " + String(code),
      });
    },
    true,
  );

  window.__yspSetNativeMode = function (enabled) {
    nativeOwnsStream = Boolean(enabled);
    if (nativeOwnsStream) {
      stopBackupPlayback();
      silenceOfficialPlayback();
    }
  };

  function prepareChannelSwitch(pid) {
    requestedPid = String(pid || "");
    captured = false;
    lastUrl = "";
    lastPid = "";
    lastDefn = "";
    lastHeartbeatTime = -1;
    focusedVideo = null;
    stopBackupPlayback();
    silenceOfficialPlayback();
    startNudge();
  }

  function findTvComponent() {
    var app =
      document.getElementById("app") ||
      document.querySelector("#app") ||
      document.body;
    var root = app && app.__vue__;
    if (!root) return null;
    var stack = [root];
    var visited = [];
    while (stack.length) {
      var vm = stack.pop();
      if (!vm || visited.indexOf(vm) >= 0) continue;
      visited.push(vm);
      if (typeof vm.changeTV === "function") return vm;
      if (vm.$children && vm.$children.length) {
        for (var i = 0; i < vm.$children.length; i += 1) {
          stack.push(vm.$children[i]);
        }
      }
    }
    return null;
  }

  function findTvChannel(vm, pid) {
    var groups = [vm.tabA, vm.tabB];
    for (var i = 0; i < groups.length; i += 1) {
      var group = groups[i];
      if (!Array.isArray(group)) continue;
      for (var j = 0; j < group.length; j += 1) {
        if (group[j] && String(group[j].pid) === pid) return group[j];
      }
    }
    return null;
  }

  function eachVueComponent(visitor) {
    var app =
      document.getElementById("app") ||
      document.querySelector("#app") ||
      document.body;
    var root = app && app.__vue__;
    if (!root) return null;
    var stack = [root];
    var visited = [];
    while (stack.length) {
      var vm = stack.pop();
      if (!vm || visited.indexOf(vm) >= 0) continue;
      visited.push(vm);
      if (visitor(vm)) return vm;
      if (vm.$children && vm.$children.length) {
        for (var i = 0; i < vm.$children.length; i += 1) {
          stack.push(vm.$children[i]);
        }
      }
    }
    return null;
  }

  function findLiveQualityPlayer(pid) {
    return eachVueComponent(function (vm) {
      var config = vm && vm.videoConfig;
      if (!config || typeof vm.getLiveUrlsByVid !== "function") return false;
      return !pid || String(config.pid) === pid;
    });
  }

  /**
   * Ask the official player to fetch a specific stream definition. The host
   * calls this to select the highest available quality and to step down when
   * native playback fails.
   */
  window.__yspApplyLiveQuality = function (definition) {
    var target = String(definition || "");
    if (!target) return false;
    stopBackupPlayback();
    var pid = requestedPid || requestedPidFromLocation();
    var vm = findLiveQualityPlayer(pid);
    if (!vm) return false;
    // A lower rung may resolve to the same URL as the initial stream. Clear
    // deduplication here so the host still receives that stream and can play it.
    captured = true;
    lastUrl = "";
    lastPid = "";
    lastDefn = "";
    stopNudge();
    try {
      var result = vm.getLiveUrlsByVid(
        String(vm.videoConfig.pid),
        vm.videoConfig.vid,
        target,
        0,
      );
      if (result && typeof result.catch === "function") {
        result.catch(function () {});
      }
      return true;
    } catch (_) {
      try {
        if (typeof vm.onChangeQuality === "function") {
          vm.onChangeQuality({ fn: target });
          return true;
        }
      } catch (_) {}
      return false;
    }
  };

  window.__yspSwitchOfficialChannel = function (pid) {
    var targetPid = String(pid || "");
    if (!targetPid) return false;
    var vm = findTvComponent();
    if (!vm) return false;
    var channel = findTvChannel(vm, targetPid);
    if (!channel) return false;
    if (vm.tvIndex && String(vm.tvIndex.pid) === targetPid) {
      requestedPid = targetPid;
      return true;
    }
    try {
      prepareChannelSwitch(targetPid);
      vm.changeTV(channel);
      return true;
    } catch (_) {
      return false;
    }
  };

  window.addEventListener("error", function (event) {
    if (captured) return;
    var message = String((event && event.message) || "page error");
    if (/AbortError/.test(message) && /play\(\) request was interrupted/.test(message)) {
      return;
    }
    post({
      type: "error",
      code: "page_error",
      message: message.slice(0, 300),
    });
  });

  window.addEventListener("unhandledrejection", function (event) {
    if (captured) return;
    var message = String((event && event.reason) || "unhandled rejection");
    if (/AbortError/.test(message) && /play\(\) request was interrupted/.test(message)) {
      return;
    }
    post({
      type: "error",
      code: "unhandled_rejection",
      message: message.slice(0, 300),
    });
  });

  function announceReady() {
    post({ type: "bridge_ready", url: location.href });
  }

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", announceReady, { once: true });
  } else {
    announceReady();
  }
})();
