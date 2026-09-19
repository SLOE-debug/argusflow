function selectVerifiedText(expected, start, end) {
  const node = this;
  if (!node.isConnected || node.textContent !== expected)
    throw new Error("SELECTION_TEXT_CHANGED");
  if (
    !Number.isInteger(start) ||
    !Number.isInteger(end) ||
    start < 0 ||
    end <= start ||
    end > expected.length
  )
    throw new Error("SELECTION_RANGE_INVALID");
  const element = node.nodeType === 1 ? node : node.parentElement;
  if (!element || element.closest("input[type=password]"))
    throw new Error("SELECTION_TARGET_INVALID");
  element.scrollIntoView({ block: "nearest", inline: "nearest" });
  const rect = element.getBoundingClientRect();
  const style = getComputedStyle(element);
  if (
    rect.width <= 0 ||
    rect.height <= 0 ||
    style.visibility !== "visible" ||
    style.display === "none"
  )
    throw new Error("SELECTION_TARGET_HIDDEN");
  const textNodes = [];
  if (node.nodeType === 3) textNodes.push(node);
  else {
    const walker = document.createTreeWalker(node, NodeFilter.SHOW_TEXT);
    while (walker.nextNode()) {
      textNodes.push(walker.currentNode);
      if (textNodes.length > 4096) throw new Error("SELECTION_BUDGET");
    }
  }
  const endpoint = (offset) => {
    let consumed = 0;
    for (const text of textNodes) {
      if (offset <= consumed + text.length) return [text, offset - consumed];
      consumed += text.length;
    }
    throw new Error("SELECTION_RANGE_STALE");
  };
  const [a, ao] = endpoint(start),
    [b, bo] = endpoint(end);
  const range = document.createRange();
  range.setStart(a, ao);
  range.setEnd(b, bo);
  const selection = document.getSelection();
  selection.removeAllRanges();
  selection.addRange(range);
  const actual = selection.toString();
  if (actual !== expected.slice(start, end))
    throw new Error("SELECTION_VERIFY_FAILED");
  return actual;
}
