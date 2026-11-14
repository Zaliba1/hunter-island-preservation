/* SPDX-License-Identifier: MPL-2.0 */
//! Guest layer points -> actual backing pixels. Logical bounds/touches stay in points.
pub fn backing_size(width:f32,height:f32,contents_scale:f32,scale_hack:u32)->Option<(u32,u32)> {
    if !contents_scale.is_finite() || contents_scale<=0.0 || contents_scale>4.0 || scale_hack==0 {return None;}
    let dimension=|points:f32| {
        let pixels=(points*contents_scale).round();
        if !pixels.is_finite() || pixels<1.0 || pixels>16384.0 {return None;}
        let result=(pixels as u32).checked_mul(scale_hack)?;
        (result<=16384).then_some(result)
    };
    Some((dimension(width)?,dimension(height)?))
}
#[cfg(test)] mod tests {
    use super::*;
    #[test] fn retina_changes_pixels_not_logical_aspect(){assert_eq!(backing_size(320.,480.,2.,1),Some((640,960)));assert_eq!(backing_size(320.,480.,1.,1),Some((320,480)));}
    #[test] fn layer_scale_and_optional_scale_hack_are_distinct(){assert_eq!(backing_size(320.,480.,2.,2),Some((1280,1920)));assert_eq!(backing_size(768.,1024.,1.,1),Some((768,1024)));}
    #[test] fn landscape_and_fractional_layer_bounds(){assert_eq!(backing_size(480.,320.,2.,1),Some((960,640)));assert_eq!(backing_size(10.25,5.25,2.,1),Some((21,11)));}
    #[test] fn invalid_or_excessive_sizes_are_rejected(){for x in [f32::NAN,f32::INFINITY,-1.,0.,20000.] {assert_eq!(backing_size(x,480.,2.,1),None);}for s in [f32::NAN,0.,-1.,5.] {assert_eq!(backing_size(320.,480.,s,1),None);}assert_eq!(backing_size(320.,480.,2.,u32::MAX),None);}
}
