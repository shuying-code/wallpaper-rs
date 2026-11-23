import Foundation
import SwiftUI

struct RootView: View {
    @StateObject private var store = WallpaperStore()

    var body: some View {
        TabView {
            DailyView()
                .environmentObject(store)
                .tabItem {
                    Label("每日", systemImage: "sun.max.fill")
                }

            GalleryView()
                .environmentObject(store)
                .tabItem {
                    Label("图库", systemImage: "square.grid.2x2.fill")
                }

            AboutView()
                .tabItem {
                    Label("关于", systemImage: "info.circle.fill")
                }
        }
        .tint(AppTheme.terracotta)
    }
}

struct DailyView: View {
    @EnvironmentObject private var store: WallpaperStore
    @State private var selected: WallpaperDetail?

    var body: some View {
        NavigationStack {
            ScrollView {
                VStack(alignment: .leading, spacing: 18) {
                    PageHeader(
                        eyebrow: "MICROSOFT BING",
                        title: "每日壁纸",
                        subtitle: "每天一张 4K 风景，保存后在照片里手动设为墙纸。"
                    )

                    if store.isLoading && store.daily.isEmpty {
                        LoadingCard(text: "正在获取必应每日壁纸…")
                    }

                    if !store.isLoading && store.daily.isEmpty {
                        RetryCard(
                            title: "暂时没有加载到每日壁纸",
                            detail: "检查网络后重试，也可以下拉刷新。",
                            actionTitle: "重新加载"
                        ) {
                            Task { await store.refreshDaily() }
                        }
                    }

                    ForEach(store.daily) { item in
                        Button {
                            selected = item.detail
                        } label: {
                            WallpaperCard(
                                title: item.title,
                                subtitle: item.copyright,
                                badge: item.date,
                                imageURL: item.thumbnailURL
                            )
                        }
                        .buttonStyle(.plain)
                    }
                }
                .padding(.horizontal, 18)
                .padding(.top, 12)
                .padding(.bottom, 28)
            }
            .background(AppTheme.background.ignoresSafeArea())
            .navigationBarTitleDisplayMode(.inline)
            .refreshable {
                await store.refreshDaily()
            }
            .task {
                await store.loadDailyIfNeeded()
            }
            .sheet(item: $selected) { detail in
                WallpaperDetailView(detail: detail)
            }
            .alert(
                "加载失败",
                isPresented: Binding(
                    get: { store.errorMessage != nil },
                    set: { if !$0 { store.errorMessage = nil } }
                )
            ) {
                Button("好", role: .cancel) {
                    store.errorMessage = nil
                }
            } message: {
                Text(store.errorMessage ?? "")
            }
        }
    }
}

struct GalleryView: View {
    @EnvironmentObject private var store: WallpaperStore
    @State private var selected: WallpaperDetail?
    @FocusState private var searchFocused: Bool

    private var rows: [[GalleryItem]] {
        stride(from: 0, to: store.gallery.count, by: 2).map { index in
            Array(store.gallery[index..<min(index + 2, store.gallery.count)])
        }
    }

    var body: some View {
        NavigationStack {
            ScrollView {
                VStack(alignment: .leading, spacing: 18) {
                    PageHeader(
                        eyebrow: "4K WALLPAPER LIBRARY",
                        title: "网上图库",
                        subtitle: "搜索必应图库，预览原图并保存到 iPhone 相册。"
                    )

                    HStack(spacing: 10) {
                        Image(systemName: "magnifyingglass")
                            .foregroundStyle(AppTheme.muted)
                        TextField("搜索森林、城市、猫…", text: $store.query)
                            .textInputAutocapitalization(.never)
                            .autocorrectionDisabled()
                            .focused($searchFocused)
                            .submitLabel(.search)
                            .onSubmit {
                                searchFocused = false
                                Task { await store.search() }
                            }
                        Button("搜索") {
                            searchFocused = false
                            Task { await store.search() }
                        }
                        .font(.subheadline.weight(.semibold))
                        .foregroundStyle(AppTheme.terracotta)
                    }
                    .padding(.horizontal, 14)
                    .padding(.vertical, 12)
                    .background(Color.white.opacity(0.9))
                    .clipShape(RoundedRectangle(cornerRadius: 16, style: .continuous))

                    if store.isLoading && store.gallery.isEmpty {
                        LoadingCard(text: "正在连接 4K 图库…")
                    }

                    if !store.isLoading && store.gallery.isEmpty {
                        RetryCard(
                            title: store.query.isEmpty ? "暂时没有加载到图库" : "没有找到匹配的壁纸",
                            detail: "检查网络或换个关键词后重试。",
                            actionTitle: "重新搜索"
                        ) {
                            Task { await store.search() }
                        }
                    }

                    LazyVStack(spacing: 14) {
                        ForEach(rows.indices, id: \.self) { index in
                            HStack(alignment: .top, spacing: 12) {
                                ForEach(rows[index]) { item in
                                    Button {
                                        selected = item.detail
                                    } label: {
                                        GalleryCard(item: item)
                                    }
                                    .buttonStyle(.plain)
                                    .frame(maxWidth: .infinity)
                                }

                                if rows[index].count == 1 {
                                    Color.clear
                                        .frame(maxWidth: .infinity)
                                }
                            }
                        }
                    }

                    if store.hasMore {
                        Button {
                            Task { await store.loadMore() }
                        } label: {
                            HStack(spacing: 8) {
                                if store.isLoadingMore {
                                    ProgressView()
                                }
                                Text(store.isLoadingMore ? "加载中…" : "加载更多")
                                    .font(.subheadline.weight(.semibold))
                            }
                            .frame(maxWidth: .infinity)
                            .padding(.vertical, 14)
                            .background(AppTheme.teal)
                            .foregroundStyle(.white)
                            .clipShape(RoundedRectangle(cornerRadius: 16, style: .continuous))
                        }
                        .disabled(store.isLoadingMore)
                    }
                }
                .padding(.horizontal, 18)
                .padding(.top, 12)
                .padding(.bottom, 28)
            }
            .background(AppTheme.background.ignoresSafeArea())
            .scrollDismissesKeyboard(.interactively)
            .navigationBarTitleDisplayMode(.inline)
            .task {
                await store.loadGalleryIfNeeded()
            }
            .sheet(item: $selected) { detail in
                WallpaperDetailView(detail: detail)
            }
            .alert(
                "加载失败",
                isPresented: Binding(
                    get: { store.errorMessage != nil },
                    set: { if !$0 { store.errorMessage = nil } }
                )
            ) {
                Button("好", role: .cancel) {
                    store.errorMessage = nil
                }
            } message: {
                Text(store.errorMessage ?? "")
            }
        }
    }
}

struct PageHeader: View {
    let eyebrow: String
    let title: String
    let subtitle: String

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text(eyebrow)
                .font(.caption.weight(.bold))
                .tracking(1.4)
                .foregroundStyle(AppTheme.terracotta)
            Text(title)
                .font(.system(size: 36, weight: .heavy, design: .rounded))
                .foregroundStyle(AppTheme.ink)
            Text(subtitle)
                .font(.subheadline)
                .foregroundStyle(AppTheme.muted)
                .fixedSize(horizontal: false, vertical: true)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }
}

struct WallpaperCard: View {
    let title: String
    let subtitle: String
    let badge: String
    let imageURL: URL

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            AsyncImage(url: imageURL) { phase in
                switch phase {
                case .success(let image):
                    image
                        .resizable()
                        .scaledToFill()
                case .failure:
                    placeholder(systemImage: "photo.badge.exclamationmark")
                case .empty:
                    placeholder(systemImage: "photo")
                        .overlay(ProgressView().tint(.white))
                @unknown default:
                    placeholder(systemImage: "photo")
                }
            }
            .frame(height: 190)
            .frame(maxWidth: .infinity)
            .clipped()

            VStack(alignment: .leading, spacing: 7) {
                HStack {
                    Text("每日精选")
                        .font(.caption.weight(.bold))
                        .foregroundStyle(AppTheme.terracotta)
                    Spacer()
                    if !badge.isEmpty {
                        Text(badge)
                            .font(.caption2.weight(.semibold))
                            .foregroundStyle(AppTheme.muted)
                    }
                }
                Text(title)
                    .font(.headline)
                    .foregroundStyle(AppTheme.ink)
                    .lineLimit(2)
                Text(subtitle)
                    .font(.caption)
                    .foregroundStyle(AppTheme.muted)
                    .lineLimit(2)
            }
            .padding(16)
        }
        .studioCard()
    }

    private func placeholder(systemImage: String) -> some View {
        ZStack {
            AppTheme.teal.opacity(0.82)
            Image(systemName: systemImage)
                .font(.largeTitle)
                .foregroundStyle(.white.opacity(0.7))
        }
    }
}

struct GalleryCard: View {
    let item: GalleryItem

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            Color.clear
                .frame(height: 150)
                .overlay {
                    AsyncImage(url: item.thumbnailURL) { phase in
                        switch phase {
                        case .success(let image):
                            image
                                .resizable()
                                .scaledToFill()
                        case .failure:
                            placeholder
                        case .empty:
                            placeholder
                                .overlay(ProgressView().tint(.white))
                        @unknown default:
                            placeholder
                        }
                    }
                }
                .clipped()

            Text(item.title.isEmpty ? "4K 壁纸" : item.title)
                .font(.subheadline.weight(.semibold))
                .foregroundStyle(AppTheme.ink)
                .lineLimit(2)
                .multilineTextAlignment(.leading)
                .frame(maxWidth: .infinity, alignment: .leading)
                .fixedSize(horizontal: false, vertical: true)
                .padding(12)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .studioCard()
    }

    private var placeholder: some View {
        ZStack {
            AppTheme.gold.opacity(0.35)
            Image(systemName: "photo")
                .font(.title)
                .foregroundStyle(AppTheme.ink.opacity(0.45))
        }
    }
}

struct LoadingCard: View {
    let text: String

    var body: some View {
        HStack(spacing: 12) {
            ProgressView()
                .tint(AppTheme.terracotta)
            Text(text)
                .font(.subheadline)
                .foregroundStyle(AppTheme.muted)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(18)
        .studioCard()
    }
}

struct RetryCard: View {
    let title: String
    let detail: String
    let actionTitle: String
    let action: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            Text(title)
                .font(.headline)
                .foregroundStyle(AppTheme.ink)
            Text(detail)
                .font(.subheadline)
                .foregroundStyle(AppTheme.muted)
                .fixedSize(horizontal: false, vertical: true)
            Button(action: action) {
                Text(actionTitle)
                    .font(.subheadline.weight(.semibold))
                    .padding(.horizontal, 16)
                    .padding(.vertical, 9)
                    .background(AppTheme.terracotta)
                    .foregroundStyle(.white)
                    .clipShape(Capsule())
            }
            .buttonStyle(.plain)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(18)
        .studioCard()
    }
}

struct WallpaperDetailView: View {
    let detail: WallpaperDetail

    @Environment(\.dismiss) private var dismiss
    @State private var isSaving = false
    @State private var resultMessage: String?
    @State private var showResult = false

    var body: some View {
        NavigationStack {
            ScrollView {
                VStack(alignment: .leading, spacing: 18) {
                    AsyncImage(url: detail.fullURL) { phase in
                        switch phase {
                        case .success(let image):
                            image
                                .resizable()
                                .scaledToFit()
                        case .failure:
                            detailPlaceholder
                        case .empty:
                            detailPlaceholder
                                .overlay(ProgressView().tint(.white))
                        @unknown default:
                            detailPlaceholder
                        }
                    }
                    .frame(maxWidth: .infinity)
                    .background(.black)
                    .clipShape(RoundedRectangle(cornerRadius: 22, style: .continuous))

                    VStack(alignment: .leading, spacing: 8) {
                        Text(detail.title)
                            .font(.title2.weight(.bold))
                            .foregroundStyle(AppTheme.ink)
                        Text(detail.subtitle)
                            .font(.subheadline)
                            .foregroundStyle(AppTheme.muted)
                        Text(detail.source)
                            .font(.caption.weight(.semibold))
                            .foregroundStyle(AppTheme.terracotta)
                    }

                    Button {
                        Task { await saveToPhotos() }
                    } label: {
                        HStack(spacing: 9) {
                            if isSaving {
                                ProgressView()
                                    .tint(.white)
                            } else {
                                Image(systemName: "square.and.arrow.down.fill")
                            }
                            Text(isSaving ? "正在保存 4K 原图…" : "保存 4K 到相册")
                                .font(.headline)
                        }
                        .frame(maxWidth: .infinity)
                        .padding(.vertical, 15)
                        .background(AppTheme.terracotta)
                        .foregroundStyle(.white)
                        .clipShape(RoundedRectangle(cornerRadius: 16, style: .continuous))
                    }
                    .disabled(isSaving)

                    if let pageURL = detail.pageURL {
                        Link(destination: pageURL) {
                            HStack {
                                Image(systemName: "safari.fill")
                                Text("打开来源网页")
                                Spacer()
                                Image(systemName: "arrow.up.right")
                            }
                            .font(.subheadline.weight(.semibold))
                            .padding(15)
                            .background(Color.white.opacity(0.85))
                            .foregroundStyle(AppTheme.teal)
                            .clipShape(RoundedRectangle(cornerRadius: 16, style: .continuous))
                        }
                    }

                    VStack(alignment: .leading, spacing: 8) {
                        Label("iOS 壁纸说明", systemImage: "info.circle.fill")
                            .font(.subheadline.weight(.bold))
                            .foregroundStyle(AppTheme.teal)
                        Text("iOS 不允许第三方 App 直接更改墙纸。保存后请打开“照片” → 选择图片 → 分享或“用作墙纸”，再按系统提示确认。")
                            .font(.footnote)
                            .foregroundStyle(AppTheme.muted)
                            .fixedSize(horizontal: false, vertical: true)
                    }
                    .padding(16)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .background(AppTheme.gold.opacity(0.18))
                    .clipShape(RoundedRectangle(cornerRadius: 16, style: .continuous))
                }
                .padding(18)
            }
            .background(AppTheme.background.ignoresSafeArea())
            .navigationTitle("预览")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .topBarTrailing) {
                    Button("关闭") {
                        dismiss()
                    }
                }
            }
            .alert("保存结果", isPresented: $showResult) {
                Button("好", role: .cancel) {}
            } message: {
                Text(resultMessage ?? "")
            }
        }
    }

    private var detailPlaceholder: some View {
        ZStack {
            AppTheme.teal
            Image(systemName: "photo")
                .font(.system(size: 52))
                .foregroundStyle(.white.opacity(0.65))
        }
        .frame(height: 260)
    }

    private func saveToPhotos() async {
        isSaving = true
        defer { isSaving = false }
        do {
            let data = try await ImageDownloader.data(from: detail.fullURL)
            try await PhotoLibrarySaver.save(data: data)
            resultMessage = "已保存到相册。打开“照片”后手动设为墙纸。"
        } catch {
            resultMessage = error.localizedDescription
        }
        showResult = true
    }
}

struct AboutView: View {
    var body: some View {
        NavigationStack {
            ScrollView {
                VStack(alignment: .leading, spacing: 18) {
                    PageHeader(
                        eyebrow: "WALLPAPER STUDIO FOR IOS",
                        title: "关于",
                        subtitle: "这是 wallpaper-rs 的 iPhone / iPad 原生入口。"
                    )

                    VStack(alignment: .leading, spacing: 14) {
                        Label("浏览必应每日壁纸与 4K 图库", systemImage: "sparkles")
                        Label("原图保存到系统相册", systemImage: "photo.on.rectangle.angled")
                        Label("按 iPhone 安全区与触控尺寸设计", systemImage: "iphone")
                    }
                    .font(.subheadline)
                    .foregroundStyle(AppTheme.ink)
                    .padding(18)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .studioCard()

                    VStack(alignment: .leading, spacing: 9) {
                        Text("为什么不能直接设置壁纸？")
                            .font(.headline)
                            .foregroundStyle(AppTheme.ink)
                        Text("iOS 没有向第三方 App 开放修改系统墙纸的能力。这个限制无法通过签名、描述文件或侧载绕过，因此 iOS 版把“保存到相册”作为最终操作。")
                            .font(.footnote)
                            .foregroundStyle(AppTheme.muted)
                            .fixedSize(horizontal: false, vertical: true)
                    }
                    .padding(18)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .background(AppTheme.teal.opacity(0.09))
                    .clipShape(RoundedRectangle(cornerRadius: 20, style: .continuous))

                    Link(destination: URL(string: "https://github.com/shuying-code/wallpaper-rs")!) {
                        HStack {
                            Image(systemName: "chevron.left.forwardslash.chevron.right")
                            Text("查看开源项目")
                            Spacer()
                            Image(systemName: "arrow.up.right")
                        }
                        .font(.subheadline.weight(.semibold))
                        .padding(16)
                        .background(Color.white.opacity(0.85))
                        .foregroundStyle(AppTheme.terracotta)
                        .clipShape(RoundedRectangle(cornerRadius: 16, style: .continuous))
                    }

                    Text("版本 \(Bundle.main.infoDictionary?["CFBundleShortVersionString"] as? String ?? "0.14.14") · 非 App Store 侧载构建")
                        .font(.caption)
                        .foregroundStyle(AppTheme.muted)
                }
                .padding(.horizontal, 18)
                .padding(.top, 12)
                .padding(.bottom, 28)
            }
            .background(AppTheme.background.ignoresSafeArea())
            .navigationBarTitleDisplayMode(.inline)
        }
    }
}
