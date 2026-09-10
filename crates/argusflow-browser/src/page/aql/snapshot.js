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
            const text = node.nodeType === 9 ? element.textContent : node.textContent;
            bytes += (text?.length || 0) * 2;
            if (bytes > 4000000 || (text?.length || 0) > 65536) throw new Error("AQL_SNAPSHOT_LIMIT");
            const bool = (property, aria) => typeof element[property] === "boolean"
                ? element[property] : element.hasAttribute(aria)
                    ? (element.getAttribute(aria) === "mixed" ? null : element.getAttribute(aria) === "true") : null;
            const attributes = Object.fromEntries(Array.from(element.attributes, a => [a.name, a.value]));
            result.push({
                path, text, value: typeof element.value === "string" ? element.value : null,
                enabled: !element.matches(":disabled") && element.getAttribute("aria-disabled") !== "true",
                visible: style.display !== "none" && style.visibility !== "hidden" && style.visibility !== "collapse" && rect.width > 0 && rect.height > 0,
                focused: node.getRootNode().activeElement === node,
                checked: bool("checked", "aria-checked"), selected: bool("selected", "aria-selected"),
                attributes, css: node.nodeType === 1 ? selectors.filter(selector => element.matches(selector)) : [],
                bounds: [rect.x, rect.y, rect.width, rect.height]
            });
        }
        // 只走真实 childNodes；文档和 Shadow 边界由 AQL 显式进入。
        for (let index = node.childNodes.length - 1; index >= 0; index--) {
            pending.push([node.childNodes[index], path + "/" + index, depth + 1]);
        }
    }
    return result;
}
