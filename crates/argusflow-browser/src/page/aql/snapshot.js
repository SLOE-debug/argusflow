function(selectors) {
    const result = [];
    const pending = [[this, "", 0]];
    let bytes = 0;
    while (pending.length) {
        const [node, path, depth] = pending.pop();
        if (result.length >= 10000 || depth > 64) throw new Error("AQL_SNAPSHOT_LIMIT");
        if ([1,3,9].includes(node.nodeType)) {
            const element = node.nodeType === 9 ? node.documentElement : node.nodeType === 1 ? node : node.parentElement || node.getRootNode().host;
            if (!element) continue;
            let rect;
            if (node.nodeType === 3) {
                const range = node.ownerDocument.createRange();
                range.selectNodeContents(node);
                rect = range.getBoundingClientRect();
            } else rect = element.getBoundingClientRect();
            const style = getComputedStyle(element);
            const rendered = style.display !== "none" && style.visibility !== "hidden" && style.visibility !== "collapse" && rect.width > 0 && rect.height > 0;
            const visible = rendered && rect.bottom > 0 && rect.right > 0 && rect.top < innerHeight && rect.left < innerWidth;
            // innerText 仅在渲染元素上读取；隐藏元素的 innerText 会退化成 textContent。
            // 名称、title、aria-label 和 input.value 均不能充当可读文字。
            const text = !rendered ? "" : node.nodeType === 3 ? node.nodeValue
                : typeof element.innerText === "string" ? element.innerText : null;
            bytes += (text?.length || 0) * 2;
            if (bytes > 4000000 || (text?.length || 0) > 65536) throw new Error("AQL_SNAPSHOT_LIMIT");
            const bool = (property, aria) => typeof element[property] === "boolean"
                ? element[property] : element.hasAttribute(aria)
                    ? (element.getAttribute(aria) === "mixed" ? null : element.getAttribute(aria) === "true") : null;
            const attributes = Object.fromEntries(Array.from(element.attributes, a => [a.name, a.value]));
            result.push({
                path, text, value: typeof element.value === "string" ? element.value : null,
                enabled: element.matches("button,input,select,textarea,option,optgroup,fieldset") || element.hasAttribute("aria-disabled")
                    ? !element.matches(":disabled") && !element.closest('[aria-disabled="true"]') : null,
                visible,
                focused: node.getRootNode().activeElement === node,
                checked: bool("checked", "aria-checked"), selected: bool("selected", "aria-selected"),
                attributes, css: node.nodeType === 1 ? selectors.filter(selector => element.matches(selector)) : [],
                bounds: node.nodeType === 9 ? [0, 0, innerWidth, innerHeight] : [rect.x, rect.y, rect.width, rect.height]
            });
        }
        // 只走真实 childNodes；文档和 Shadow 边界由 AQL 显式进入。
        for (let index = node.childNodes.length - 1; index >= 0; index--) {
            pending.push([node.childNodes[index], path + "/" + index, depth + 1]);
        }
    }
    return result;
}
