//! 一对一词汇表；仅 WASM 编辑服务依赖中文拼写。
const WORDS: &[(&str, &str)] = &[
    ("element", "元素"),
    ("window", "窗口"),
    ("dialog", "对话框"),
    ("pane", "面板"),
    ("button", "按钮"),
    ("textbox", "输入框"),
    ("checkbox", "复选框"),
    ("radio", "单选框"),
    ("combobox", "组合框"),
    ("list", "列表"),
    ("list_item", "列表项"),
    ("tree", "树"),
    ("tree_item", "树项"),
    ("tab", "选项卡组"),
    ("tab_item", "选项卡"),
    ("menu", "菜单"),
    ("menu_item", "菜单项"),
    ("link", "链接"),
    ("image", "图片"),
    ("table", "表格"),
    ("row", "行"),
    ("cell", "单元格"),
    ("document", "文档"),
    ("text", "文本"),
    ("name", "名称"),
    ("key", "标识"),
    ("value", "值"),
    ("enabled", "可用"),
    ("visible", "可见"),
    ("focused", "聚焦"),
    ("checked", "勾选"),
    ("selected", "选中"),
    ("confidence", "置信度"),
    ("uia.automation_id", "界面.自动化标识"),
    ("uia.class_name", "界面.类名"),
    ("uia.accelerator_key", "界面.快捷键"),
    ("uia.access_key", "界面.访问键"),
    ("uia.framework_id", "界面.框架标识"),
    ("dom.id", "网页.标识"),
    ("dom.test_id", "网页.测试标识"),
    ("dom.class", "网页.类名"),
    ("dom.tag", "网页.标签"),
    ("first", "首个"),
    ("nth", "第几个"),
    ("css", "CSS"),
    ("frame", "框架"),
    ("shadow", "影子根"),
    ("contains", "包含"),
    ("starts_with", "开头为"),
    ("ends_with", "结尾为"),
    ("matches", "匹配"),
    ("and", "且"),
    ("or", "或"),
    ("not", "非"),
    ("true", "真"),
    ("false", "假"),
];
pub(crate) fn chinese(word: &str) -> &str {
    WORDS
        .iter()
        .find(|(english, _)| *english == word)
        .map(|(_, chinese)| *chinese)
        .unwrap_or(word)
}
pub(crate) fn english(word: &str) -> &str {
    WORDS
        .iter()
        .find(|(_, chinese)| *chinese == word)
        .map(|(english, _)| *english)
        .unwrap_or(word)
}
