import AppKit
import NotekiCore
import WebKit

/// The editor's web view, handing to CodeMirror the Edit menu actions WebKit would get wrong:
/// CodeMirror draws only the lines on screen, so WebKit's select-all and undo would see a part
/// of the Note. Adapted from MarkEdit's EditorWebView (MIT, see THIRD_PARTY.md).
final class EditorWebView: WKWebView {
    var onCommand: (EditorCommand) -> Void = { _ in }
    /// False while no Note is in front (an Attachment, a placeholder): the menus leave it alone.
    var takesCommands = false

    override func selectAll(_ sender: Any?) {
        onCommand(.selectAll)
    }

    @objc func undo(_ sender: Any?) {
        onCommand(.undo)
    }

    @objc func redo(_ sender: Any?) {
        onCommand(.redo)
    }

    override func validateUserInterfaceItem(_ item: any NSValidatedUserInterfaceItem) -> Bool {
        switch item.action {
        case #selector(undo(_:)), #selector(redo(_:)), #selector(selectAll(_:)): takesCommands
        default: super.validateUserInterfaceItem(item)
        }
    }
}
