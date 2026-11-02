import AppKit
import NotekiCore
import SwiftUI

/// Noteki's own tab bar, in the toolbar's band above the Note (ADR 0003): a click brings a Tab in front, its × or a middle click closes it.
struct TabBar: View {
    let model: AppModel
    /// Where each Tab is in the bar, scrolled as shown: a middle click finds its Tab here.
    @State private var frames: [NotekiCore.Tab.ID: CGRect] = [:]

    var body: some View {
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: 0) {
                ForEach(model.tabs.all) { tab in
                    TabButton(
                        title: tab.title,
                        isInFront: tab.id == model.tabs.front?.id,
                        select: { model.select(tab.id) },
                        close: { model.closeTab(tab.id) }
                    )
                    .onGeometryChange(for: CGRect.self) { $0.frame(in: .named(TabBar.space)) } action: { frames[tab.id] = $0 }
                    Divider().frame(height: 16)
                }
            }
        }
        .frame(height: 30)
        .padding(.horizontal, 8)
        .frame(maxHeight: .infinity)
        .coordinateSpace(.named(TabBar.space))
        // Behind the whole bar, not each Tab: a view of AppKit inside a Tab took its first click.
        .background(MiddleClick { point in
            guard let tab = model.tabs.all.first(where: { frames[$0.id]?.contains(point) == true }) else { return false }
            model.closeTab(tab.id)
            return true
        })
    }

    private static let space = "TabBar"
}

private struct TabButton: View {
    let title: String
    let isInFront: Bool
    let select: () -> Void
    let close: () -> Void
    @State private var isHovered = false

    var body: some View {
        HStack(spacing: 4) {
            Button(action: close) {
                Image(systemName: "xmark")
                    .font(.system(size: 9, weight: .semibold))
                    .frame(width: 16, height: 16)
                    .contentShape(.rect)
            }
            .buttonStyle(.plain)
            .foregroundStyle(.secondary)
            // In front or under the pointer, as in Safari: a click elsewhere on the Tab selects it.
            .opacity(isInFront || isHovered ? 1 : 0)
            .help("Close Tab")
            .accessibilityLabel("Close \(title)")
            Text(title)
                .lineLimit(1)
                .truncationMode(.middle)
                .foregroundStyle(isInFront ? .primary : .secondary)
            Spacer(minLength: 0)
        }
        .font(.callout)
        .padding(.horizontal, 8)
        .frame(minWidth: 120, maxWidth: 220, maxHeight: .infinity)
        .background(isInFront ? AnyShapeStyle(.quaternary) : AnyShapeStyle(.clear), in: .rect(cornerRadius: 7))
        .contentShape(.rect)
        .onTapGesture(perform: select)
        .onHover { isHovered = $0 }
        .accessibilityElement(children: .contain)
        .accessibilityAddTraits(isInFront ? [.isSelected, .isButton] : .isButton)
        .accessibilityLabel(title)
        .accessibilityAction(.default, select)
    }
}

/// A middle click on the view it is behind, where it is from the view's top left; `action` says whether it used it.
/// SwiftUI never sees the middle button.
private struct MiddleClick: NSViewRepresentable {
    let action: (CGPoint) -> Bool

    func makeNSView(context: Context) -> MiddleClickView {
        MiddleClickView()
    }

    func updateNSView(_ view: MiddleClickView, context: Context) {
        view.action = action
    }
}

private final class MiddleClickView: NSView {
    var action: (CGPoint) -> Bool = { _ in false }
    private var monitor: Any?

    // From the top left, as SwiftUI measures.
    override var isFlipped: Bool { true }

    override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        if let monitor { NSEvent.removeMonitor(monitor) }
        monitor = nil
        guard window != nil else { return }
        monitor = NSEvent.addLocalMonitorForEvents(matching: .otherMouseUp) { [weak self] event in
            guard let self, event.buttonNumber == 2, event.window === self.window else { return event }
            let point = self.convert(event.locationInWindow, from: nil)
            return self.bounds.contains(point) && self.action(point) ? nil : event
        }
    }

    // Clicks go through to the Tabs: only the monitor listens.
    override func hitTest(_ point: NSPoint) -> NSView? { nil }
}
