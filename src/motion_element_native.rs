use crate::{Frame, Motion, MotionState, StyleFrame, StyleMotion, StyleMotionState};
use gpui::{
    AnyElement, App, Bounds, Element, ElementId, Empty, GlobalElementId, InspectorElementId,
    IntoElement, LayoutId, Styled, Window,
};

/// Applies a sampled motion frame to an element.
pub trait MotionApply<E> {
    /// Applies the sampled values for this frame.
    fn apply(&mut self, element: E, frame: Frame) -> E;
}

/// The default MotionApply implementation, which uses Frame::apply.
#[doc(hidden)]
pub struct DefaultMotionApply;

impl<E: Styled> MotionApply<E> for DefaultMotionApply {
    fn apply(&mut self, element: E, frame: Frame) -> E {
        frame.apply(element)
    }
}

impl<E, F: FnMut(E, Frame) -> E> MotionApply<E> for F {
    fn apply(&mut self, element: E, frame: Frame) -> E {
        self(element, frame)
    }
}

/// A styled element animated by a retained, ID-scoped MotionState.
///
/// State is retained by GPUI's keyed element state, so callers can declare
/// targets each render without owning animation tasks. Keep id stable for the
/// logical element. Use map to apply a frame to custom values and style_motion
/// for independent style properties.
pub struct MotionElement<E, F = DefaultMotionApply> {
    id: ElementId,
    element: Option<E>,
    motion: Motion,
    style_motion: Option<StyleMotion>,
    apply: F,
}

impl<E: Styled + IntoElement + 'static> MotionElement<E> {
    /// Wraps a styled element with a declarative motion target.
    pub fn new(id: impl Into<ElementId>, element: E, motion: Motion) -> Self {
        Self {
            id: id.into(),
            element: Some(element),
            motion,
            style_motion: None,
            apply: DefaultMotionApply,
        }
    }
}

impl<E, F> MotionElement<E, F> {
    /// Applies a custom presentation mapping for each sampled frame.
    pub fn map<G: MotionApply<E>>(self, apply: G) -> MotionElement<E, G> {
        MotionElement {
            id: self.id,
            element: self.element,
            motion: self.motion,
            style_motion: self.style_motion,
            apply,
        }
    }

    /// Adds independently retained style motion to this element.
    pub fn style_motion(mut self, motion: StyleMotion) -> Self {
        self.style_motion = Some(motion);
        self
    }
}

#[derive(Default)]
struct RetainedMotion {
    motion: MotionState,
    style: Option<StyleMotionState>,
}

impl<E, F> IntoElement for MotionElement<E, F>
where
    E: Styled + IntoElement + 'static,
    F: MotionApply<E> + 'static,
{
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl<E, F> Element for MotionElement<E, F>
where
    E: Styled + IntoElement + 'static,
    F: MotionApply<E> + 'static,
{
    type RequestLayoutState = AnyElement;
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        Some(self.id.clone())
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        id: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, AnyElement) {
        let (frame, style_frame): (Frame, Option<StyleFrame>) = window.with_element_state(
            id.expect("motion element has an ID"),
            |state: Option<RetainedMotion>, window| {
                let mut state = state.unwrap_or_default();
                let frame = state.motion.frame(self.motion, window, cx);
                let style_frame = self.style_motion.map(|motion| {
                    state
                        .style
                        .get_or_insert_with(StyleMotionState::default)
                        .frame(motion, window, cx)
                });
                ((frame, style_frame), state)
            },
        );

        let child = self.element.take().expect("motion layout requested once");
        let mut child = if frame.is_mounted() {
            let child = self.apply.apply(child, frame);
            match style_frame {
                Some(style) => style.apply(child).into_any_element(),
                None => child.into_any_element(),
            }
        } else {
            Empty.into_any_element()
        };

        let layout = child.request_layout(window, cx);
        (layout, child)
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<gpui::Pixels>,
        child: &mut AnyElement,
        window: &mut Window,
        cx: &mut App,
    ) {
        child.prepaint(window, cx);
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<gpui::Pixels>,
        child: &mut AnyElement,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        child.paint(window, cx);
    }
}
