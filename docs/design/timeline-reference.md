# 时间轴设计参考

使用内置 imagegen 生成 `timeline-reference.png`，原图 2172 × 724。

## 图像分析与实现尺寸

- 外框约占图像 x=26–2145、y=85–638；白底、冷灰细边、圆角约 16px。实现使用 8px 圆角，跟随桌面逻辑像素密度。
- 顶栏约 110px、摘要栏约 86px、时间尺及双轨约 260px、底栏约 94px；以约 0.5 比例落地为紧凑工作区。
- 左侧实心蓝色播放按钮与等宽时间码，右侧灰底分段按钮。
- 摘要左侧键盘图标、时间、按键和窗口；右侧灰色来源徽标与定位属性。
- 时间尺有主次刻度；键盘与鼠标各占一条水平细线轨道，紫色与灰色圆角标记。
- 选区跨两条轨道，浅蓝填充，两端蓝色竖边与白色握柄。播放游标是珊瑚色细线，上方显示同色时间胶囊。
- 底栏标签浅底蓝色描边，关闭图标与标签在一起；调整与处理操作右对齐。

## 生成提示词

Use case: ui-mockup. Create a high fidelity flat desktop UI design reference for ArgusFlow recording playback TIMELINE dock only, wide 3:1 image. Modern premium professional light theme, precise implementable CSS geometry, no perspective, no device frame, no decorative background. White surface subtle cool gray border, small 8px radii, restrained indigo accent, thin coral playhead. Compact functional layout: top toolbar 40px with indigo play icon button at left, monospace time 00:07.011 / 00:12.433, right segmented controls 浏览画面 and 选择时间段. Second 36px row current event summary with tiny purple keyboard icon, 00:06.485, Ctrl + V, gray window name 搜索框; small UIA badge and AutomationId: SearchBox on right. Then beautiful accurate horizontal time ruler with main and minor ticks, labels 00:00, 00:02, 00:04, 00:06, 00:08, 00:10, 00:12; tick labels never collide. Below ruler two very thin labeled lanes: 键盘 purple little rounded event blocks, 鼠标 gray little rounded event blocks. A selected interval from 5.5s to 8.0s is translucent indigo with fine border and elegant small white grip lines at both ends. Coral playhead spans ruler and tracks at 7.0s, small time pill 00:07.011 above. Bottom 36px minimal selection row with outlined chip 00:05.500–00:08.000 and inline x; right 调整时间 and compact privacy treatment dropdown 遮盖截图 followed by subtle action button 处理所选内容. Chinese sans serif typography sharp legible, font sizes 11-13px equivalents, calm hierarchy, whitespace efficient. No previous/next buttons, no event count, no huge blue area, no gradients, no fake waveform, no thumbnails, no giant title. Show exactly one coherent production-ready timeline dock filling image, polished as a real advanced editor.
