//! 不可变 GPU 分块快照，索引分组写时复制，未变化纹理由相邻版本共享。
use super::{Graphics, Texture, invalid};
use argusflow_capture_contracts::{CaptureResult, PixelRect};
use std::sync::Arc;
use windows::Win32::Graphics::Direct3D11::{D3D11_BOX, ID3D11Texture2D};

/// 索引每组 64 项，修改一个块不复制整屏索引。
const INDEX_GROUP: usize = 64;
pub(in crate::capture) const TILE: u32 = 32;
#[derive(Clone)]
pub(in crate::capture) struct Tile {
    pub texture: Arc<Texture>,
    pub x: u32,
    pub y: u32,
}
#[derive(Clone)]
pub(in crate::capture) struct TileMap {
    pub width: u32,
    pub height: u32,
    groups: Vec<Arc<Vec<Tile>>>,
}
impl TileMap {
    pub fn initial(texture: Arc<Texture>) -> Self {
        let width = texture.width;
        let height = texture.height;
        let mut tiles = Vec::new();
        for y in (0..height).step_by(32) {
            for x in (0..width).step_by(32) {
                tiles.push(Tile {
                    texture: texture.clone(),
                    x,
                    y,
                });
            }
        }
        Self {
            width,
            height,
            groups: tiles
                .chunks(INDEX_GROUP)
                .map(|chunk| Arc::new(chunk.to_vec()))
                .collect(),
        }
    }
    pub fn update(&mut self, index: usize, tile: Tile) {
        Arc::make_mut(&mut self.groups[index / INDEX_GROUP])[index % INDEX_GROUP] = tile;
    }
    fn tile(&self, index: usize) -> &Tile {
        &self.groups[index / INDEX_GROUP][index % INDEX_GROUP]
    }
    pub fn crop(&self, graphics: &Graphics, region: PixelRect) -> CaptureResult<Arc<Texture>> {
        let bounds = PixelRect::new(0, 0, self.width, self.height)?;
        if !bounds.contains(region) {
            return Err(invalid("snapshot crop bounds"));
        }
        let target = Arc::new(Texture::new(
            graphics,
            region.width(),
            region.height(),
            false,
        )?);
        for index in tiles_for_regions(self.width, self.height, &[region])? {
            let columns = self.width.div_ceil(TILE);
            let x = index as u32 % columns * TILE;
            let y = index as u32 / columns * TILE;
            let tile_bounds =
                PixelRect::new(x, y, TILE.min(self.width - x), TILE.min(self.height - y))?;
            if let Some(intersection) = region.intersection(tile_bounds) {
                let tile = self.tile(index);
                let source = D3D11_BOX {
                    left: tile.x + intersection.x() - x,
                    top: tile.y + intersection.y() - y,
                    front: 0,
                    right: tile.x + intersection.right() - x,
                    bottom: tile.y + intersection.bottom() - y,
                    back: 1,
                };
                // SAFETY: 交集同时在源块和目标区域内，全部资源同适配器。
                unsafe {
                    graphics.context.CopySubresourceRegion(
                        &target.native,
                        0,
                        intersection.x() - region.x(),
                        intersection.y() - region.y(),
                        0,
                        &tile.texture.native,
                        0,
                        Some(&source),
                    );
                }
            }
        }
        Ok(target)
    }
}
/// 一批冻结候选块，差分完成后才挑选真实改变的块进入新索引。
pub(in crate::capture) struct TileBatch {
    pub atlas: Arc<Texture>,
    pub indices: Vec<usize>,
    pub locations: Vec<[u32; 4]>,
}
impl TileBatch {
    pub fn freeze(
        graphics: &Graphics,
        source: &ID3D11Texture2D,
        width: u32,
        height: u32,
        indices: &[usize],
    ) -> CaptureResult<Self> {
        if indices.is_empty() || indices.len() > 2048 {
            return Err(invalid("tile batch size"));
        }
        let columns = (indices.len() as u32).min(32);
        let atlas = Arc::new(Texture::new(
            graphics,
            columns * TILE,
            (indices.len() as u32).div_ceil(columns) * TILE,
            false,
        )?);
        let mut locations = Vec::with_capacity(indices.len());
        for (slot, index) in indices.iter().enumerate() {
            let x = *index as u32 % width.div_ceil(TILE) * TILE;
            let y = *index as u32 / width.div_ceil(TILE) * TILE;
            if x >= width || y >= height {
                return Err(invalid("tile index"));
            }
            let ax = slot as u32 % columns * TILE;
            let ay = slot as u32 / columns * TILE;
            let area = D3D11_BOX {
                left: x,
                top: y,
                front: 0,
                right: (x + TILE).min(width),
                bottom: (y + TILE).min(height),
                back: 1,
            };
            // SAFETY: 候选块来自校验后的屏幕范围，atlas 槽宽高为 32。
            unsafe {
                graphics.context.CopySubresourceRegion(
                    &atlas.native,
                    0,
                    ax,
                    ay,
                    0,
                    source,
                    0,
                    Some(&area),
                );
            }
            locations.push([x, y, ax, ay]);
        }
        Ok(Self {
            atlas,
            indices: indices.to_vec(),
            locations,
        })
    }
    pub fn apply(&self, map: &mut TileMap, changed: &[bool]) {
        for ((index, location), changed) in self.indices.iter().zip(&self.locations).zip(changed) {
            if *changed {
                map.update(
                    *index,
                    Tile {
                        texture: self.atlas.clone(),
                        x: location[2],
                        y: location[3],
                    },
                );
            }
        }
    }
    /// 将稀疏变化压紧到新 atlas，避免一个变化块长期固定整批候选像素。
    pub fn apply_compact(
        &self,
        graphics: &Graphics,
        map: &mut TileMap,
        changed: &[bool],
    ) -> CaptureResult<()> {
        let count = changed.iter().filter(|yes| **yes).count();
        if count == 0 {
            return Ok(());
        }
        if count == changed.len() {
            self.apply(map, changed);
            return Ok(());
        }
        let columns = (count as u32).min(32);
        let texture = Arc::new(Texture::new(
            graphics,
            columns * 32,
            (count as u32).div_ceil(columns) * 32,
            false,
        )?);
        for (slot, ((index, location), _)) in self
            .indices
            .iter()
            .zip(&self.locations)
            .zip(changed)
            .filter(|(_, yes)| **yes)
            .enumerate()
        {
            let x = slot as u32 % columns * 32;
            let y = slot as u32 / columns * 32;
            let area = D3D11_BOX {
                left: location[2],
                top: location[3],
                front: 0,
                right: location[2] + 32.min(map.width - location[0]),
                bottom: location[3] + 32.min(map.height - location[1]),
                back: 1,
            };
            // SAFETY: 源块来自同批 atlas，目标槽按实际变化块数分配。
            unsafe {
                graphics.context.CopySubresourceRegion(
                    &texture.native,
                    0,
                    x,
                    y,
                    0,
                    &self.atlas.native,
                    0,
                    Some(&area),
                );
            }
            map.update(
                *index,
                Tile {
                    texture: texture.clone(),
                    x,
                    y,
                },
            );
        }
        Ok(())
    }
}
/// 候选块位图去重，重叠区域不会重复比较像素。
pub(in crate::capture) fn tiles_for_regions(
    width: u32,
    height: u32,
    regions: &[PixelRect],
) -> CaptureResult<Vec<usize>> {
    let bounds = PixelRect::new(0, 0, width, height)?;
    let columns = width.div_ceil(TILE) as usize;
    let rows = height.div_ceil(TILE) as usize;
    let mut selected = vec![false; columns * rows];
    for region in regions {
        if !bounds.contains(*region) {
            return Err(invalid("candidate outside source"));
        }
        for y in region.y() / TILE..region.bottom().div_ceil(TILE) {
            for x in region.x() / TILE..region.right().div_ceil(TILE) {
                selected[y as usize * columns + x as usize] = true;
            }
        }
    }
    Ok(selected
        .into_iter()
        .enumerate()
        .filter_map(|(index, yes)| yes.then_some(index))
        .collect())
}
