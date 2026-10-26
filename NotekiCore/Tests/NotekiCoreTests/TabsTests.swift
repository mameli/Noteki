import Foundation
import Testing
import NotekiCore

@Suite("Tabs")
struct TabsTests {
    /// What each Tab shows, the one in front marked with `*`.
    private func shown(_ tabs: Tabs) -> [String] {
        tabs.all.map { ($0.id == tabs.front?.id ? "*" : "") + $0.path }
    }

    @Test("a Note opens in the Tab in front, or in a new Tab after it; the first opens a Tab")
    func open() {
        var tabs = Tabs()
        #expect(tabs.front == nil)

        tabs.show("a.md")
        tabs.show("b.md")
        #expect(shown(tabs) == ["*b.md"])
        tabs.show("c.md", inNewTab: true)
        #expect(shown(tabs) == ["b.md", "*c.md"])
        tabs.select(tabs.all[0].id)
        tabs.show("d.md", inNewTab: true)
        #expect(shown(tabs) == ["b.md", "*d.md", "c.md"])
    }

    @Test("back and forward follow what a Tab showed; showing another drops what was forward")
    func history() {
        var tabs = Tabs()
        tabs.show("a.md")
        tabs.show("b.md")
        tabs.show("c.md")
        #expect(tabs.front?.canGoForward == false)

        tabs.back()
        tabs.back()
        #expect(shown(tabs) == ["*a.md"])
        #expect(tabs.front?.canGoBack == false)
        tabs.forward()
        #expect(shown(tabs) == ["*b.md"])
        tabs.show("d.md")
        #expect(tabs.front?.canGoForward == false)
        tabs.back()
        #expect(shown(tabs) == ["*b.md"])
        // The same again is no step.
        tabs.show("b.md")
        tabs.back()
        #expect(shown(tabs) == ["*a.md"])
    }

    @Test("closing a Tab brings the one after it, or before it when it was the last")
    func close() {
        var tabs = Tabs()
        tabs.show("a.md")
        tabs.show("b.md", inNewTab: true)
        tabs.show("c.md", inNewTab: true)
        tabs.select(tabs.all[1].id)

        tabs.close(tabs.front!.id)
        #expect(shown(tabs) == ["a.md", "*c.md"])
        tabs.close(tabs.front!.id)
        #expect(shown(tabs) == ["*a.md"])
        tabs.close(tabs.front!.id)
        #expect(shown(tabs) == [])
        #expect(tabs.front == nil)
    }

    @Test("closing a Tab behind keeps the one in front")
    func closeBehind() {
        var tabs = Tabs()
        tabs.show("a.md")
        tabs.show("b.md", inNewTab: true)

        tabs.close(tabs.all[0].id)
        #expect(shown(tabs) == ["*b.md"])
    }

    @Test("next and previous go round")
    func cycle() {
        var tabs = Tabs()
        tabs.show("a.md")
        tabs.show("b.md", inNewTab: true)
        tabs.show("c.md", inNewTab: true)

        tabs.selectNext()
        #expect(shown(tabs) == ["*a.md", "b.md", "c.md"])
        tabs.selectPrevious()
        #expect(shown(tabs) == ["a.md", "b.md", "*c.md"])
    }

    @Test("a Followed rename changes the path in every Tab and its history, folders included")
    func renamed() {
        var tabs = Tabs()
        tabs.show("Inbox/a.md")
        tabs.show("b.md")
        tabs.show("Inbox/a.md", inNewTab: true)

        tabs.renamed(from: "Inbox/a.md", to: "Meetings/A.md")
        #expect(shown(tabs) == ["b.md", "*Meetings/A.md"])
        tabs.select(tabs.all[0].id)
        tabs.back()
        #expect(shown(tabs) == ["*Meetings/A.md", "Meetings/A.md"])
        tabs.renamed(from: "Meetings", to: "Old")
        #expect(shown(tabs) == ["*Old/A.md", "Old/A.md"])
    }

    @Test("the Tabs come back as they were, ready to be kept in the preferences")
    func restored() throws {
        var tabs = Tabs()
        tabs.show("a.md")
        tabs.show("b.md")
        tabs.show("c.md", inNewTab: true)
        tabs.select(tabs.all[0].id)

        let data = try JSONEncoder().encode(tabs)
        let back = try JSONDecoder().decode(Tabs.self, from: data)
        #expect(shown(back) == ["*b.md", "c.md"])
        #expect(back.front?.canGoBack == true)
        #expect(back == tabs)
    }

    @Test("a Tab is called by its Note's name, or its Attachment's file name")
    func title() {
        var tabs = Tabs()
        tabs.show("Meetings/Retro 2026.md")
        tabs.show("Media/photo.png", inNewTab: true)
        tabs.show("Notes.MD", inNewTab: true)

        #expect(tabs.all.map(\.title) == ["Retro 2026", "photo.png", "Notes"])
    }

    @Test("a Tab whose file is gone at launch is left out; the others come back")
    func gone() {
        var tabs = Tabs()
        tabs.show("a.md")
        tabs.show("gone.md", inNewTab: true)
        tabs.show("b.md", inNewTab: true)

        let kept = tabs.keeping { $0 != "gone.md" }
        #expect(shown(kept) == ["a.md", "*b.md"])
    }
}
