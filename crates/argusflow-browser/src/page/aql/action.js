function(action, point) {
    const element = this.nodeType === 9 ? this.documentElement : this.nodeType === 1 ? this : this.parentElement || this.getRootNode().host;
    if (!this.isConnected || !element) throw new Error("AQL_DETACHED");
    const doc = this.nodeType === 9 ? this : this.ownerDocument;
    const view = doc.defaultView;
    const deepestHit = (x, y) => {
        let hit = doc.elementFromPoint(x, y);
        while (hit?.shadowRoot) {
            const next = hit.shadowRoot.elementFromPoint(x, y);
            if (!next || next === hit) break;
            hit = next;
        }
        return hit;
    };
    if (action === "focus") {
        const editable = element.isContentEditable || element.tagName === "TEXTAREA"
            || (element.tagName === "INPUT" && ["text","search","email","password","url","tel","number"].includes(element.type));
        if (!editable || element.disabled || element.readOnly) return false;
        element.focus({preventScroll: true});
        if (element.isContentEditable) {
            const selection = doc.getSelection();
            if (!selection || !element.contains(selection.anchorNode) || !element.contains(selection.focusNode)) return false;
            selection.collapseToEnd();
        } else if (typeof element.selectionEnd === "number") {
            element.setSelectionRange(element.selectionEnd, element.selectionEnd);
        } else {
            // number/email 等控件不提供安全的光标选区接口，不能保证保留已有内容。
            return false;
        }
        return element.getRootNode().activeElement === element;
    }
    if (action === "frame") {
        let supported = true;
        let current = element;
        while (current) {
            const style = view.getComputedStyle(current);
            if (style.perspective !== "none") supported = false;
            if (style.transform !== "none") {
                const matrix = new view.DOMMatrixReadOnly(style.transform);
                if (!matrix.is2D || matrix.b !== 0 || matrix.c !== 0 || matrix.a <= 0 || matrix.d <= 0) supported = false;
            }
            current = current.parentElement || current.getRootNode().host;
        }
        const rect = element.getBoundingClientRect();
        return {left:rect.left,top:rect.top,width:rect.width,height:rect.height,
            border_left:element.clientLeft,border_top:element.clientTop,
            layout_width:element.offsetWidth,layout_height:element.offsetHeight,supported};
    }
    if (action === "hit") {
        const hit = deepestHit(point[0], point[1]);
        return hit === element || element.contains(hit);
    }
    let rect;
    if (this.nodeType === 3) {
        const range = doc.createRange();
        range.selectNodeContents(this);
        rect = range.getBoundingClientRect();
    } else rect = element.getBoundingClientRect();
    const left = Math.max(0, rect.left), top = Math.max(0, rect.top);
    const right = Math.min(view.innerWidth, rect.right), bottom = Math.min(view.innerHeight, rect.bottom);
    if (right <= left || bottom <= top || element.matches(":disabled") || element.getAttribute("aria-disabled") === "true") return null;
    const x = (left + right) / 2, y = (top + bottom) / 2;
    const hit = deepestHit(x, y);
    return hit === element || element.contains(hit) ? [x, y] : null;
}
