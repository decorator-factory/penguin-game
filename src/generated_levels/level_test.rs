#![allow(clippy::excessive_precision, clippy::pedantic)]
/// Generated from file `test_level.tmj` on `2025-11-27 15:21:37.348202+00:00`
use crate::levels;
use glam::vec2;

#[rustfmt::skip]
pub fn build(mut builder: levels::LevelBuilder) -> levels::Level {
    builder.rect(Some("bricks"), vec2(-540., 120.), vec2(4476., 24.));
    builder.rect(Some("barrier"), vec2(-504., -2400.), vec2(24., 2040.));
    builder.rect(Some("barrier"), vec2(3936., -2400.), vec2(24., 2544.));
    builder.polygon_graphics("wood_dark", &[vec2(-348., 120.), vec2(-324., 72.), vec2(-312., 72.), vec2(-336., 120.), vec2(-348., 120.)]);
    builder.polygon_graphics("wood_dark", &[vec2(-276., 120.), vec2(-300., 72.), vec2(-312., 72.), vec2(-288., 120.), vec2(-276., 120.)]);
    builder.rect_graphics("bricks_dark", vec2(24., 60.), vec2(72., 60.));
    builder.rect(Some("bricks"), vec2(12., 36.), vec2(96., 24.));
    builder.rect(Some("caution"), vec2(96., 60.), vec2(12., 60.));
    builder.rect(Some("barrier"), vec2(-504., -276.), vec2(24., 396.));
    builder.rect_graphics("barrier", vec2(-504., -360.), vec2(24., 84.));
    builder.rect_graphics("wood_dark", vec2(-480., -300.), vec2(132., 12.));
    builder.rect_graphics("wood_dark", vec2(-456., -288.), vec2(12., 12.));
    builder.rect_graphics("wood_dark", vec2(-384., -288.), vec2(12., 12.));
    builder.polygon(Some("caution"), &[vec2(-468., -228.), vec2(-468., -276.), vec2(-360., -276.), vec2(-336., -252.), vec2(-360., -228.)]);
    builder.rect(Some("barrier"), vec2(-1452., -1188.), vec2(12., 1668.));
    builder.rect_graphics("bricks_dark", vec2(-600., 720.), vec2(72., 60.));
    builder.rect(Some("bricks"), vec2(-624., 780.), vec2(264., 12.));
    builder.polygon(Some("barrier"), &[vec2(-624., 792.), vec2(-624., 780.), vec2(-1440., 468.), vec2(-1440., 480.)]);
    builder.rect(None, vec2(-360., 144.), vec2(12., 648.));
    builder.polygon_graphics("wood_dark", &[vec2(-612., 720.), vec2(-516., 720.), vec2(-564., 672.)]);
    builder.polygon_graphics("bricks_dark", &[vec2(-528., 708.), vec2(-540., 696.), vec2(-540., 672.), vec2(-528., 672.), vec2(-528., 708.)]);
    builder.rect(Some("bricks"), vec2(-720., 612.), vec2(48., 12.));
    builder.polygon(Some("caution"), &[vec2(-348., 96.), vec2(-348., 48.), vec2(-276., 48.), vec2(-264., 72.), vec2(-276., 96.)]);
    builder.rect_graphics("bricks_dark", vec2(444., -48.), vec2(72., 168.));
    builder.rect(Some("bricks"), vec2(432., -72.), vec2(96., 24.));
    builder.rect(Some("caution"), vec2(516., -48.), vec2(12., 168.));
    builder.rect_graphics("bricks_dark", vec2(864., -96.), vec2(72., 216.));
    builder.rect(Some("bricks"), vec2(852., -120.), vec2(96., 24.));
    builder.rect(Some("caution"), vec2(936., -96.), vec2(12., 216.));
    builder.rect(Some("barrier"), vec2(600., -600.), vec2(24., 480.));
    builder.rect(Some("barrier"), vec2(1056., -600.), vec2(24., 456.));
    builder.rect_graphics("bricks_dark", vec2(1272., -156.), vec2(72., 276.));
    builder.rect(Some("bricks"), vec2(1260., -180.), vec2(96., 24.));
    builder.rect(Some("caution"), vec2(1344., -156.), vec2(12., 276.));
    builder.rect(Some("barrier"), vec2(1416., -600.), vec2(24., 456.));
    builder.rect_graphics("bricks_dark", vec2(1584., -180.), vec2(72., 300.));
    builder.rect(Some("bricks"), vec2(1572., -204.), vec2(96., 24.));
    builder.rect(Some("caution"), vec2(1656., -180.), vec2(12., 300.));
    builder.rect(Some("barrier"), vec2(1728., -1200.), vec2(24., 1056.));
    builder.polygon_trigger(levels::TriggerKind::Hello, &[vec2(552., -600.), vec2(588., -720.), vec2(672., -600.)]);
    builder.polygon_graphics("barrier", &[vec2(552., -600.), vec2(588., -720.), vec2(672., -600.)]);
    builder.polygon_graphics("caution", &[vec2(-471.06, 717.709), vec2(-467.746, 706.33), vec2(-461.78, 706.883), vec2(-460.675, 719.146)]);
    builder.polygon(Some("caution"), &[vec2(-471.088, 717.709), vec2(-474.506, 722.559), vec2(-474.833, 730.405), vec2(-473.671, 741.172), vec2(-466.365, 753.421), vec2(-455.138, 765.247), vec2(-443.435, 771.668), vec2(-430.074, 776.041), vec2(-460.455, 719.367)]);
    builder.polygon(Some("caution"), &[vec2(-450.424, 739.077), vec2(-435.926, 750.978), vec2(-421.477, 753.977), vec2(-429.852, 775.93)]);
    builder.polygon(Some("caution"), &[vec2(-422.893, 754.), vec2(-397.925, 750.3), vec2(-384.005, 748.753), vec2(-380.594, 751.209), vec2(-380.912, 756.486), vec2(-389.971, 767.313), vec2(-417.7, 776.814), vec2(-429.853, 776.151)]);
    builder.rect(Some("bricks_dark"), vec2(-468.685, 699.48), vec2(8.037, 7.706));
    builder.polygon_trigger(levels::TriggerKind::Panic, &[vec2(-474.667, 723.667), vec2(-468.333, 699.667), vec2(-460.667, 699.333), vec2(-380., 749.333), vec2(-378.667, 761.667), vec2(-416.333, 780.333), vec2(-453.333, 769.333), vec2(-475.333, 743.333)]);
    builder.rect_trigger(levels::TriggerKind::Hello, vec2(24., 60.), vec2(72., 60.));
    builder.level_start(vec2(-120., 96.));
    builder.build_or_die()
}
