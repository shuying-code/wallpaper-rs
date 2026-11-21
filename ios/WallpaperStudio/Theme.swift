import SwiftUI

enum AppTheme {
    static let background = Color(red: 0.96, green: 0.94, blue: 0.89)
    static let surface = Color.white.opacity(0.86)
    static let ink = Color(red: 0.17, green: 0.14, blue: 0.12)
    static let muted = Color(red: 0.43, green: 0.39, blue: 0.34)
    static let terracotta = Color(red: 0.76, green: 0.34, blue: 0.20)
    static let teal = Color(red: 0.08, green: 0.34, blue: 0.33)
    static let gold = Color(red: 0.88, green: 0.66, blue: 0.27)
}

extension View {
    func studioCard() -> some View {
        background(AppTheme.surface)
            .clipShape(RoundedRectangle(cornerRadius: 24, style: .continuous))
            .shadow(color: .black.opacity(0.07), radius: 18, y: 8)
    }
}
