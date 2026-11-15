/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! Basic legacy table views: data-source rows, custom cells and scroll content.
//! Reuse caching, headers, editing and transition animations are not modeled.
use super::UIScrollViewHostObject;
use crate::Environment;
use crate::frameworks::core_graphics::{CGFloat, CGPoint, CGRect, CGSize};
use crate::frameworks::foundation::{NSInteger, ns_string::get_static_str};
use crate::objc::{autorelease, id, impl_HostObject_with_superclass, msg, msg_class,
    msg_super, nil, objc_classes, release, retain, ClassExports, NSZonePtr};

#[derive(Default)]
struct Table {
    superclass: UIScrollViewHostObject,
    data_source: id, // weak, just like UIScrollView's delegate
    row_height: CGFloat,
    rows: Vec<(id, NSInteger, NSInteger)>, // cells owned by the subview hierarchy
    reloading: bool,
    needs_reload: bool,
    allows_selection: Option<bool>,
}
impl_HostObject_with_superclass!(Table);

#[derive(Default)]
struct Cell {
    superclass: super::super::UIViewHostObject,
    content: id, // owned by the subview hierarchy
    reuse_identifier: id,
}
impl_HostObject_with_superclass!(Cell);

fn reload(env: &mut Environment, table: id) {
    let state=env.objc.borrow_mut::<Table>(table);
    if state.reloading {return;}
    state.reloading=true;state.needs_reload=false;
    let old=std::mem::take(&mut state.rows);
    let source=state.data_source;
    for (cell,_,_) in old {()=msg![env; cell removeFromSuperview];}
    let sections:NSInteger=if source==nil {0} else if env.objc.object_has_method_named(&env.mem,source,"numberOfSectionsInTableView:") {
        msg![env; source numberOfSectionsInTableView:table]
    } else {1};
    assert!((0..=1024).contains(&sections),"Unsupported table section count");
    let delegate:id=msg![env; table delegate];
    let bounds:CGRect=msg![env; table bounds];
    let default_height=env.objc.borrow::<Table>(table).row_height;
    let default_height=if default_height>0. {default_height} else {44.};
    let mut y=0.;
    let mut cells=Vec::new();
    for section in 0..sections {
        let count:NSInteger=msg![env; source tableView:table numberOfRowsInSection:section];
        assert!((0..=10000).contains(&count) && cells.len()+count as usize<=10000,"Unsupported table row count");
        for row in 0..count {
            let path:id=msg_class![env; NSIndexPath indexPathForRow:row inSection:section];
            let height:CGFloat=if delegate!=nil && env.objc.object_has_method_named(&env.mem,delegate,"tableView:heightForRowAtIndexPath:") {
                msg![env; delegate tableView:table heightForRowAtIndexPath:path]
            } else {default_height};
            assert!(height.is_finite() && height>=0.,"Invalid table row height");
            let cell:id=msg![env; source tableView:table cellForRowAtIndexPath:path];
            assert!(cell!=nil,"Table data source returned a nil cell");
            let frame=CGRect{origin:CGPoint{x:0.,y},size:CGSize{width:bounds.size.width,height}};
            ()=msg![env; cell setFrame:frame];()=msg![env; cell layoutSubviews];
            ()=msg![env; table addSubview:cell];
            cells.push((cell,section,row));y+=height;
        }
    }
    ()=msg![env; table setContentSize:(CGSize{width:bounds.size.width,height:y})];
    let state=env.objc.borrow_mut::<Table>(table);state.rows=cells;state.reloading=false;
    ()=msg![env; table setNeedsDisplay];
}

pub const CLASSES: ClassExports = objc_classes! {
(env, this, _cmd);
@implementation UITableView: UIScrollView
+ (id)allocWithZone:(NSZonePtr)_zone {
    env.objc.alloc_object(this,Box::<Table>::default(),&mut env.mem)
}
- (id)initWithFrame:(CGRect)frame style:(NSInteger)_style {
    msg_super![env; this initWithFrame:frame]
}
- (id)initWithCoder:(id)coder {
    let this:id=msg_super![env; this initWithCoder:coder];
    let key=get_static_str(env,"UIRowHeight");
    let height:f32=msg![env; coder decodeFloatForKey:key];
    env.objc.borrow_mut::<Table>(this).row_height=height;
    ()=msg![env; this setClipsToBounds:true];
    this
}
- (())awakeFromNib {env.objc.borrow_mut::<Table>(this).needs_reload=true;}
- (id)dataSource {env.objc.borrow::<Table>(this).data_source}
- (())setDataSource:(id)source {
    let state=env.objc.borrow_mut::<Table>(this);state.data_source=source;state.needs_reload=true;
}
- (bool)allowsSelection {env.objc.borrow::<Table>(this).allows_selection.unwrap_or(true)}
- (())setAllowsSelection:(bool)allows {env.objc.borrow_mut::<Table>(this).allows_selection=Some(allows);}
- (())touchesEnded:(id)touches withEvent:(id)_event {
    let allows:bool=msg![env; this allowsSelection];if !allows{return;}
    let all:id=msg![env; touches allObjects];let count:u32=msg![env; all count];if count==0{return;}
    let touch:id=msg![env; all objectAtIndex:0u32];
    let point:CGPoint=msg![env; touch locationInView:this];
    let rows=env.objc.borrow::<Table>(this).rows.clone();
    for (cell,section,row) in rows {
        let frame:CGRect=msg![env; cell frame];
        if point.x>=frame.origin.x && point.x<frame.origin.x+frame.size.width &&
           point.y>=frame.origin.y && point.y<frame.origin.y+frame.size.height {
            let delegate:id=msg![env; this delegate];
            if delegate!=nil && env.objc.object_has_method_named(&env.mem,delegate,"tableView:didSelectRowAtIndexPath:") {
                let path:id=msg_class![env; NSIndexPath indexPathForRow:row inSection:section];
                ()=msg![env; delegate tableView:this didSelectRowAtIndexPath:path];
            }
            break;
        }
    }
}
- (())setRowHeight:(CGFloat)height {env.objc.borrow_mut::<Table>(this).row_height=height;}
- (CGFloat)rowHeight {let h=env.objc.borrow::<Table>(this).row_height;if h>0. {h} else {44.}}
- (())setSeparatorStyle:(NSInteger)_style {}
- (())reloadData {reload(env,this);}
- (id)dequeueReusableCellWithIdentifier:(id)_identifier {nil}
- (id)cellForRowAtIndexPath:(id)path {
    let section:NSInteger=msg![env; path section];let row:NSInteger=msg![env; path row];
    env.objc.borrow::<Table>(this).rows.iter().find(|(_,s,r)| *s==section && *r==row).map(|(c,_,_)|*c).unwrap_or(nil)
}
- (id)indexPathForCell:(id)cell {
    let found=env.objc.borrow::<Table>(this).rows.iter().find(|(c,_,_)|*c==cell).copied();
    if let Some((_,section,row))=found {msg_class![env; NSIndexPath indexPathForRow:row inSection:section]} else {nil}
}
- (())layoutSubviews {
    if env.objc.borrow::<Table>(this).needs_reload {reload(env,this);}
    let bounds:CGRect=msg![env; this bounds];
    let rows=env.objc.borrow::<Table>(this).rows.clone();
    for (cell,_,_) in rows {
        let mut frame:CGRect=msg![env; cell frame];frame.size.width=bounds.size.width;
        ()=msg![env; cell setFrame:frame];()=msg![env; cell layoutSubviews];
    }
}
@end
@implementation UITableViewCell: UIView
+ (id)allocWithZone:(NSZonePtr)_zone {
    env.objc.alloc_object(this,Box::<Cell>::default(),&mut env.mem)
}
- (id)initWithStyle:(NSInteger)_style reuseIdentifier:(id)identifier {
    let _:id=msg![env; this initWithFrame:(CGRect::default())];
    retain(env,identifier);env.objc.borrow_mut::<Cell>(this).reuse_identifier=identifier;
    this
}
- (id)reuseIdentifier {env.objc.borrow::<Cell>(this).reuse_identifier}
- (id)contentView {
    let content=env.objc.borrow::<Cell>(this).content;
    if content!=nil {return content;}
    let bounds:CGRect=msg![env; this bounds];
    let view:id=msg_class![env; UIView alloc];let view:id=msg![env; view initWithFrame:bounds];
    env.objc.borrow_mut::<Cell>(this).content=view;
    ()=msg![env; this addSubview:view];release(env,view);
    view
}
- (())layoutSubviews {
    let view=env.objc.borrow::<Cell>(this).content;
    let bounds:CGRect=msg![env; this bounds];()=msg![env; view setFrame:bounds];
}
- (())setSelectionStyle:(NSInteger)_style {}
- (())dealloc {
    let identifier=env.objc.borrow::<Cell>(this).reuse_identifier;
    release(env,identifier);
    msg_super![env; this dealloc]
}
@end

@implementation _touchHLE_TableProbeSource: NSObject
- (NSInteger)numberOfSectionsInTableView:(id)_table {2}
- (NSInteger)tableView:(id)_table numberOfRowsInSection:(NSInteger)_section {2}
- (CGFloat)tableView:(id)_table heightForRowAtIndexPath:(id)path {
    let section:NSInteger=msg![env; path section];60.+10.*section as f32
}
- (id)tableView:(id)_table cellForRowAtIndexPath:(id)path {
    let row:NSInteger=msg![env; path row];let section:NSInteger=msg![env; path section];
    let cell:id=msg_class![env; UITableViewCell alloc];
    let cell:id=msg![env; cell initWithStyle:0i32 reuseIdentifier:nil];
    ()=msg![env; cell setTag:(section*10+row)];
    autorelease(env,cell)
}
@end
};

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn data_source_rows_index_paths_resize_and_reload_preserve_ownership() {
        let mut environment=Environment::new_without_app(
            crate::options::Options{headless:true,..Default::default()},
            crate::image::Image::from_pixel_vec(vec![0;4],(1,1)),
        ).unwrap();let env=&mut environment;
        let pool:id=msg_class![env; NSAutoreleasePool new];
        let source:id=msg_class![env; _touchHLE_TableProbeSource new];
        let table:id=msg_class![env; UITableView alloc];
        let table:id=msg![env; table initWithFrame:(CGRect{origin:CGPoint::default(),size:CGSize{width:300.,height:200.}}) style:0i32];
        ()=msg![env; table setDataSource:source];()=msg![env; table setDelegate:source];
        let allows:bool=msg![env; table allowsSelection];assert!(allows);
        ()=msg![env; table setAllowsSelection:false];
        let allows:bool=msg![env; table allowsSelection];assert!(!allows);
        ()=msg![env; table layoutSubviews];
        let size:CGSize=msg![env; table contentSize];assert_eq!(size,CGSize{width:300.,height:260.});
        let path:id=msg_class![env; NSIndexPath indexPathForRow:1i32 inSection:1i32];
        let cell:id=msg![env; table cellForRowAtIndexPath:path];assert_ne!(cell,nil);
        let tag:NSInteger=msg![env; cell tag];assert_eq!(tag,11);
        let found:id=msg![env; table indexPathForCell:cell];
        let equal:bool=msg![env; found isEqual:path];assert!(equal);
        let frame:CGRect=msg![env; cell frame];assert_eq!({frame.origin.y},190.);assert_eq!({frame.size.height},70.);
        retain(env,cell);
        ()=msg![env; table setBounds:(CGRect{origin:CGPoint::default(),size:CGSize{width:500.,height:200.}})];
        ()=msg![env; table layoutSubviews];
        let frame:CGRect=msg![env; cell frame];assert_eq!({frame.size.width},500.);
        ()=msg![env; table reloadData];
        let parent:id=msg![env; cell superview];assert_eq!(parent,nil);
        let replacement:id=msg![env; table cellForRowAtIndexPath:path];assert_ne!(replacement,cell);
        ()=msg![env; table setDataSource:nil];()=msg![env; table reloadData];
        let size:CGSize=msg![env; table contentSize];assert_eq!({size.height},0.);
        let missing:id=msg![env; table cellForRowAtIndexPath:path];assert_eq!(missing,nil);
        release(env,cell);release(env,table);release(env,source);release(env,pool);
    }
}
