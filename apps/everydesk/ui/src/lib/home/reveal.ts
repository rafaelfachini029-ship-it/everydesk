// When the contact cards under the field fold away (tested in tests/GoField.test.ts).
// WKWebView (Tauri on macOS) does not focus a <button> on click, so a click on a card blurs
// the field with relatedTarget null: a pointer pressed inside the area must keep it open.

export function shouldCollapse(s: { focusWithin: boolean; pointerInside: boolean }): boolean {
  return !s.focusWithin && !s.pointerInside
}
