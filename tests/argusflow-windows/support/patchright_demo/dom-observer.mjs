/** 只读观察：不接受条目选择器、示范序号、目标文字或业务步骤。 */
import { appendFileSync } from 'node:fs';
export async function observe(context, journal) {
  await context.exposeBinding('__argusEvidence', (_source, value) => {
    appendFileSync(journal, JSON.stringify(value) + '\n');
  });
  await context.addInitScript(() => {
    function path(node) {
      if (node?.nodeType === Node.TEXT_NODE) node = node.parentElement;
      if (!(node instanceof Element)) return null;
      const parts = [];
      while (node && node.nodeType === Node.ELEMENT_NODE) {
        const siblings = node.parentElement ? [...node.parentElement.children].filter(e => e.tagName === node.tagName) : [node];
        parts.unshift(`${node.tagName.toLowerCase()}[${siblings.indexOf(node)+1}]`);
        node = node.parentElement;
      }
      return '/' + parts.join('/');
    }
    for (const type of ['pointerdown','pointerup','copy','paste']) {
      document.addEventListener(type, event => {
        const selection = getSelection();
        const anchor = selection?.anchorNode;
        const node = anchor?.nodeType === Node.TEXT_NODE ? anchor.parentElement : anchor;
        const rect = node instanceof Element ? node.getBoundingClientRect() : null;
        globalThis.__argusEvidence({epoch_ms:Date.now(),type,url:location.href,title:document.title,trusted:event.isTrusted,
          x:event.clientX??null,y:event.clientY??null,button:event.button??null,
          selection:selection?.toString()??'',xpath:path(anchor),anchor_offset:selection?.anchorOffset,focus_offset:selection?.focusOffset,
          node_text:node?.textContent??null,rect:rect?[rect.x,rect.y,rect.width,rect.height]:null,
          viewport:[innerWidth,innerHeight],clipboard_text:event.clipboardData?.getData('text/plain')??null}).catch(()=>{});
      },true);
    }
  });
}
