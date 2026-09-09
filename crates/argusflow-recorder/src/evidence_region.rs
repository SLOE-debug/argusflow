//! 变化范围与事件窗口相交后裁剪；始终保持虚拟屏幕物理坐标。
use argusflow_core::{EvidenceFrame, InspectionFailure, InspectionRect, ScreenPoint};
use std::sync::Arc;

/// 屏幕坐标矩形交集；不产生负尺寸。
pub(crate) fn intersect(a: InspectionRect, b: InspectionRect) -> Option<InspectionRect> {
    let (x, y) = (a.x.max(b.x), a.y.max(b.y));
    let right = (a.x + a.width).min(b.x + b.width);
    let bottom = (a.y + a.height).min(b.y + b.height);
    (right > x && bottom > y).then_some(InspectionRect {
        x,
        y,
        width: right - x,
        height: bottom - y,
    })
}

/// 累积本事件窗口内的变化，其他窗口变化不能扩大裁剪。
pub(crate) fn include(bounds: &mut Option<InspectionRect>, rect: InspectionRect) {
    *bounds = Some(match *bounds {
        None => rect,
        Some(old) => {
            let x = old.x.min(rect.x);
            let y = old.y.min(rect.y);
            InspectionRect {
                x,
                y,
                width: (old.x + old.width).max(rect.x + rect.width) - x,
                height: (old.y + old.height).max(rect.y + rect.height) - y,
            }
        }
    });
}

/// 变化区域加 24px 语境；点击点也必须可见。没有变化时使用事件窗口。
pub(crate) fn crop(
    frame: &Arc<EvidenceFrame>,
    scope: InspectionRect,
    changed: Option<InspectionRect>,
    pointer: Option<ScreenPoint>,
) -> Result<Arc<EvidenceFrame>, InspectionFailure> {
    let scope = intersect(scope, frame.bounds()).ok_or(InspectionFailure::InvalidGeometry)?;
    let mut region = changed.and_then(|rect| intersect(rect, scope));
    if let Some(point) = pointer.filter(|point| scope.contains(*point)) {
        if region.is_some() {
            include(
                &mut region,
                InspectionRect {
                    x: point.x.into(),
                    y: point.y.into(),
                    width: 1.0,
                    height: 1.0,
                },
            );
        }
    }
    let region = region
        .map(|rect| InspectionRect {
            x: rect.x - 24.0,
            y: rect.y - 24.0,
            width: rect.width + 48.0,
            height: rect.height + 48.0,
        })
        .unwrap_or(scope);
    let region = intersect(region, scope).ok_or(InspectionFailure::InvalidGeometry)?;
    let left = (region.x - frame.bounds().x).floor().max(0.0) as u32;
    let top = (region.y - frame.bounds().y).floor().max(0.0) as u32;
    let right = ((region.x + region.width - frame.bounds().x).ceil() as u32).min(frame.width());
    let bottom = ((region.y + region.height - frame.bounds().y).ceil() as u32).min(frame.height());
    if right <= left || bottom <= top {
        return Err(InspectionFailure::InvalidGeometry);
    }
    if left == 0 && top == 0 && right == frame.width() && bottom == frame.height() {
        return Ok(frame.clone());
    }
    let (width, height) = (right - left, bottom - top);
    let mut pixels = Vec::with_capacity(width as usize * height as usize * 4);
    for row in top..bottom {
        let start = (row as usize * frame.width() as usize + left as usize) * 4;
        pixels.extend_from_slice(&frame.pixels()[start..start + width as usize * 4]);
    }
    EvidenceFrame::new(
        InspectionRect {
            x: frame.bounds().x + f64::from(left),
            y: frame.bounds().y + f64::from(top),
            width: width.into(),
            height: height.into(),
        },
        width,
        height,
        frame.format(),
        pixels,
    )
    .map(Arc::new)
}

/// 保留块位置；屏幕几何改变时整个新桌面属于变化。
pub(crate) fn changes(
    previous: Option<&EvidenceFrame>,
    current: &EvidenceFrame,
) -> Vec<InspectionRect> {
    let Some(previous) = previous.filter(|previous| {
        previous.bounds() == current.bounds()
            && previous.width() == current.width()
            && previous.height() == current.height()
            && previous.format() == current.format()
    }) else {
        return vec![current.bounds()];
    };
    // EvidenceFrame 构造阶段已经验证像素布局；共享精确差分保留细小输入反馈。
    fn view(frame: &EvidenceFrame) -> Result<argusflow_capture::PixelView<'_>, InspectionFailure> {
        argusflow_capture::PixelView::new(
            frame.pixels(),
            frame.width(),
            frame.height(),
            frame.width() as usize * 4,
            frame.format(),
        )
    }
    let result = view(previous)
        .and_then(|old| view(current).and_then(|new| argusflow_capture::compare(old, new, None)));
    match result {
        Ok(changes) => changes
            .regions()
            .iter()
            .map(|rect| InspectionRect {
                x: current.bounds().x + f64::from(rect.x),
                y: current.bounds().y + f64::from(rect.y),
                width: f64::from(rect.width),
                height: f64::from(rect.height),
            })
            .collect(),
        // 非法布局不能被解释为没有变化。
        Err(_) => vec![current.bounds()],
    }
}
