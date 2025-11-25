#![allow(clippy::excessive_precision, clippy::pedantic)]
/// Generated from file `test_level.tmj` on `2025-11-26 11:47:53.708345+00:00`
use crate::levels;
use glam::vec2;

pub fn build(mut builder: levels::LevelBuilder) -> levels::Level {
    builder.rect(Some("bricks"), vec2(-540.0, 120.0), vec2(4476.0, 24.0));
    builder.rect(Some("barrier"), vec2(-504.0, -2400.0), vec2(24.0, 2040.0));
    builder.rect(Some("barrier"), vec2(3936.0, -2400.0), vec2(24.0, 2544.0));
    builder.polygon_graphics("wood_dark", &[
        vec2(-348.0, 120.0),
        vec2(-324.0, 72.0),
        vec2(-312.0, 72.0),
        vec2(-336.0, 120.0),
        vec2(-348.0, 120.0),
    ]);
    builder.polygon_graphics("wood_dark", &[
        vec2(-276.0, 120.0),
        vec2(-300.0, 72.0),
        vec2(-312.0, 72.0),
        vec2(-288.0, 120.0),
        vec2(-276.0, 120.0),
    ]);
    builder.rect_graphics("bricks_dark", vec2(24.0, 60.0), vec2(72.0, 60.0));
    builder.rect(Some("bricks"), vec2(12.0, 36.0), vec2(96.0, 24.0));
    builder.rect(Some("caution"), vec2(96.0, 60.0), vec2(12.0, 60.0));
    builder.rect(Some("barrier"), vec2(-504.0, -276.0), vec2(24.0, 396.0));
    builder.rect_graphics("barrier", vec2(-504.0, -360.0), vec2(24.0, 84.0));
    builder.rect_graphics("wood_dark", vec2(-480.0, -300.0), vec2(132.0, 12.0));
    builder.rect_graphics("wood_dark", vec2(-456.0, -288.0), vec2(12.0, 12.0));
    builder.rect_graphics("wood_dark", vec2(-384.0, -288.0), vec2(12.0, 12.0));
    builder.polygon(Some("caution"), &[
        vec2(-468.0, -228.0),
        vec2(-468.0, -276.0),
        vec2(-360.0, -276.0),
        vec2(-336.0, -252.0),
        vec2(-360.0, -228.0),
    ]);
    builder.rect(Some("barrier"), vec2(-1452.0, -1188.0), vec2(12.0, 1668.0));
    builder.rect_graphics("bricks_dark", vec2(-600.0, 720.0), vec2(72.0, 60.0));
    builder.rect(Some("bricks"), vec2(-624.0, 780.0), vec2(264.0, 12.0));
    builder.polygon(Some("barrier"), &[
        vec2(-624.0, 792.0),
        vec2(-624.0, 780.0),
        vec2(-1440.0, 468.0),
        vec2(-1440.0, 480.0),
    ]);
    builder.rect(None, vec2(-360.0, 144.0), vec2(12.0, 648.0));
    builder.polygon_graphics("wood_dark", &[
        vec2(-612.0, 720.0),
        vec2(-516.0, 720.0),
        vec2(-564.0, 672.0),
    ]);
    builder.polygon_graphics("bricks_dark", &[
        vec2(-528.0, 708.0),
        vec2(-540.0, 696.0),
        vec2(-540.0, 672.0),
        vec2(-528.0, 672.0),
        vec2(-528.0, 708.0),
    ]);
    builder.rect(Some("bricks"), vec2(-720.0, 612.0), vec2(48.0, 12.0));
    builder.polygon(Some("caution"), &[
        vec2(-348.0, 96.0),
        vec2(-348.0, 48.0),
        vec2(-276.0, 48.0),
        vec2(-264.0, 72.0),
        vec2(-276.0, 96.0),
    ]);
    builder.rect_graphics("bricks_dark", vec2(444.0, -48.0), vec2(72.0, 168.0));
    builder.rect(Some("bricks"), vec2(432.0, -72.0), vec2(96.0, 24.0));
    builder.rect(Some("caution"), vec2(516.0, -48.0), vec2(12.0, 168.0));
    builder.rect_graphics("bricks_dark", vec2(864.0, -96.0), vec2(72.0, 216.0));
    builder.rect(Some("bricks"), vec2(852.0, -120.0), vec2(96.0, 24.0));
    builder.rect(Some("caution"), vec2(936.0, -96.0), vec2(12.0, 216.0));
    builder.rect(Some("barrier"), vec2(600.0, -600.0), vec2(24.0, 480.0));
    builder.rect(Some("barrier"), vec2(1056.0, -600.0), vec2(24.0, 456.0));
    builder.rect_graphics("bricks_dark", vec2(1272.0, -156.0), vec2(72.0, 276.0));
    builder.rect(Some("bricks"), vec2(1260.0, -180.0), vec2(96.0, 24.0));
    builder.rect(Some("caution"), vec2(1344.0, -156.0), vec2(12.0, 276.0));
    builder.rect(Some("barrier"), vec2(1416.0, -600.0), vec2(24.0, 456.0));
    builder.rect_graphics("bricks_dark", vec2(1584.0, -180.0), vec2(72.0, 300.0));
    builder.rect(Some("bricks"), vec2(1572.0, -204.0), vec2(96.0, 24.0));
    builder.rect(Some("caution"), vec2(1656.0, -180.0), vec2(12.0, 300.0));
    builder.rect(Some("barrier"), vec2(1728.0, -1200.0), vec2(24.0, 1056.0));
    builder.polygon_graphics("caution", &[
        vec2(-483.458095238095, 720.228571428572),
        vec2(-480.14380952380924, 708.8495238095243),
        vec2(-474.17809523809495, 709.4019047619053),
        vec2(-473.073333333333, 721.6647619047625),
    ]);
    builder.polygon(Some("caution"), &[
        vec2(-483.485714285714, 720.228571428572),
        vec2(-486.9035064935062, 725.0779220779226),
        vec2(-487.23038961038935, 732.9238961038966),
        vec2(-486.06818181818153, 743.69064935065),
        vec2(-478.76285714285683, 755.9400000000006),
        vec2(-467.5355844155841, 767.7656277056285),
        vec2(-455.8321212121208, 774.1870129870138),
        vec2(-442.4714285714281, 778.5600000000007),
        vec2(-472.8523809523806, 721.8857142857148),
    ]);
    builder.polygon(Some("caution"), &[
        vec2(-462.8215584415587, 741.5968831168836),
        vec2(-448.32357686530514, 753.4976948783053),
        vec2(-433.8750649350652, 756.4969696969702),
        vec2(-442.25047619047643, 778.4495238095243),
    ]);
    builder.polygon(Some("caution"), &[
        vec2(-435.29047619047645, 756.52),
        vec2(-410.3228571428572, 752.8190476190476),
        vec2(-396.4028571428572, 751.2723809523809),
        vec2(-392.9919480519481, 753.7281385281385),
        vec2(-393.3095238095239, 759.0057142857144),
        vec2(-402.3685714285715, 769.832380952381),
        vec2(-430.0980952380955, 779.3333333333335),
        vec2(-442.25047619047643, 778.6704761904763),
    ]);
    builder.rect(
        Some("bricks_dark"),
        vec2(-481.082857142857, 702.0),
        vec2(8.03714285714289, 7.70571428571432),
    );
    builder.level_start(vec2(-120.0, 96.0));
    builder.build_or_die()
}
