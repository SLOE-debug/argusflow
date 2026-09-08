function () {
    // 固定只读 inspector：与 AQL DOM matcher 使用相同角色和 accessible-name 规则。
    const element = this.nodeType === Node.ELEMENT_NODE ? this : this.parentElement;
    if (!element) return null;
    const role = (node) => {
        const explicit = (node.getAttribute('role') || '').toLowerCase().replaceAll('-', '_');
        const aliases = { textbox: 'text_box', checkbox: 'check_box', listitem: 'list_item',
            treeitem: 'tree_item', tablist: 'tab', menuitem: 'menu_item', img: 'image',
            grid: 'table', gridcell: 'cell' };
        if (explicit) return aliases[explicit] || explicit;
        const tag = node.localName;
        if (tag === 'input') {
            if (['button', 'submit', 'reset'].includes(node.type)) return 'button';
            if (node.type === 'checkbox') return 'check_box';
            if (node.type === 'radio') return 'radio';
            return 'text_box';
        }
        if (tag === 'a' && node.hasAttribute('href')) return 'link';
        const roles = { button: 'button', textarea: 'text_box', select: 'combo_box',
            ul: 'list', ol: 'list', li: 'list_item', table: 'table', tr: 'row', td: 'cell',
            th: 'cell', img: 'image', nav: 'menu', body: 'document', html: 'document' };
        return roles[tag] || (node.children.length === 0 ? 'text' : 'pane');
    };
    const sensitive = (node) => node.type === 'password' || node.matches('[data-sensitive], [data-private]')
        || /password|passwd|passcode|secret|token|api.?key|cc-number|cc-csc|card.?number|cvv|cvc|one-time-code|otp|ssn|密码|验证码|密钥|身份证|银行卡/i
            .test(['id', 'name', 'autocomplete', 'aria-label', 'data-testid'].map(key => node.getAttribute(key) || '').join(' '));
    const name = (node) => {
        const labelled = (node.getAttribute('aria-labelledby') || '').split(/\s+/).filter(Boolean)
            .map(id => node.ownerDocument.getElementById(id)?.textContent || '').join(' ').trim();
        if (labelled) return labelled;
        if (node.getAttribute('aria-label')) return node.getAttribute('aria-label').trim();
        if (node.labels?.length) return Array.from(node.labels).map(label => label.textContent || '').join(' ').trim();
        // 不从可编辑区域读取 innerText/textContent（contenteditable 的值也属于输入）。
        return (node.getAttribute('alt') || node.getAttribute('title') ||
            (node.isContentEditable || ['input', 'textarea'].includes(node.localName) ? '' :
                node.innerText || node.textContent || '')).replace(/\s+/g, ' ').trim();
    };
    const snapshot = (node) => ({
        role: role(node), name: name(node) || null,
        automation_id: null, test_id: node.getAttribute('data-testid'), stable_id: node.id || null,
        class_name: typeof node.className === 'string' ? node.className : null, framework_id: null,
    });
    const ancestors = [];
    for (let parent = element.parentElement; parent && ancestors.length < 8; parent = parent.parentElement) {
        ancestors.push(snapshot(parent));
    }
    const rect = element.getBoundingClientRect();
    return { semantics: snapshot(element), ancestors,
        bounds: { x: rect.x, y: rect.y, width: rect.width, height: rect.height },
        editable: element.isContentEditable || ['input', 'textarea'].includes(element.localName),
        sensitive: sensitive(element) || Boolean(element.closest('[data-sensitive], [data-private], input[type=password]')),
        // Shadow DOM 和 iframe 容器仍是有效观察事实；只拒绝未变换的子文档坐标。
        top_level_viewport: element.ownerDocument.defaultView === element.ownerDocument.defaultView.top,
    };
}
