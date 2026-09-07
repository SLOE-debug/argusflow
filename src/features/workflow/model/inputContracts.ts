/** 组合键可使用的修饰键。 */
export type KeyboardModifier = 'control' | 'alt' | 'shift';

/** 不依赖输入法布局的主键。 */
export type KeyboardKey =
  | { type: 'enter' }
  | { type: 'escape' }
  | { type: 'tab' }
  | { type: 'backspace' }
  | { type: 'delete' }
  | { type: 'arrow_left' }
  | { type: 'arrow_right' }
  | { type: 'arrow_up' }
  | { type: 'arrow_down' }
  | { type: 'home' }
  | { type: 'end' }
  | { type: 'page_up' }
  | { type: 'page_down' }
  | { type: 'character'; value: string };

/** 一次完整按下并逆序释放的组合键。 */
export type KeyChord = {
  key: KeyboardKey;
  modifiers: KeyboardModifier[];
};
