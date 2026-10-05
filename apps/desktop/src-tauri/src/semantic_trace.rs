use compiler_core::{
    semantic::{MAX_TRACE_EVENTS, SemanticTrace, TraceEventKind},
    source::SourceMap,
};
use serde::Serialize;

use crate::{LocationDto, location};

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TraceAttributeDto {
    expression: usize,
    type_label: String,
    location: LocationDto,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TraceEventDto {
    pub kind: &'static str,
    expression: usize,
    pub location: LocationDto,
    rule: String,
    depth_before: usize,
    pub depth_after: usize,
    pop_count: usize,
    pushed: Option<TraceAttributeDto>,
    conversion_type: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SemanticTraceDto {
    pub events: Vec<TraceEventDto>,
    pub truncated: bool,
}

pub(crate) fn trace_dto(trace: SemanticTrace, sources: &SourceMap, path: &str) -> SemanticTraceDto {
    let truncated = trace.truncated || trace.events.len() > MAX_TRACE_EVENTS;
    let events = trace
        .events
        .into_iter()
        .take(MAX_TRACE_EVENTS)
        .map(|event| TraceEventDto {
            kind: match event.kind {
                TraceEventKind::Push => "push",
                TraceEventKind::Reduce => "reduce",
                TraceEventKind::Consume => "consume",
                TraceEventKind::Conversion => "conversion",
                TraceEventKind::Recover => "recover",
            },
            expression: event.expression.0,
            location: location(sources, event.span, path),
            rule: event.rule.chars().take(80).collect(),
            depth_before: event.depth_before,
            depth_after: event.depth_after,
            pop_count: event.pop_count,
            pushed: event.pushed.map(|attribute| TraceAttributeDto {
                expression: attribute.expression.0,
                type_label: attribute.type_label.chars().take(80).collect(),
                location: location(sources, attribute.span, path),
            }),
            conversion_type: event
                .conversion_type
                .map(|ty| ty.chars().take(80).collect()),
        })
        .collect();
    SemanticTraceDto { events, truncated }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounds_projection_and_preserves_conversion_deltas() {
        let (analysis, trace) = compiler_core::analyze_with_trace("2 + 0.5dec;", MAX_TRACE_EVENTS);
        let mut trace = trace.unwrap();
        let conversion = trace
            .events
            .iter()
            .find(|event| event.kind == TraceEventKind::Conversion)
            .unwrap()
            .clone();
        trace.events = vec![conversion; MAX_TRACE_EVENTS + 1];
        trace.events[0].conversion_type = Some("é".repeat(100));
        let dto = trace_dto(trace, &analysis.sources, "lesson.itd");
        assert!(dto.truncated);
        assert_eq!(dto.events.len(), MAX_TRACE_EVENTS);
        let event = &dto.events[0];
        assert_eq!(event.kind, "conversion");
        assert_eq!(event.depth_before, event.depth_after);
        assert_eq!(event.pop_count, 0);
        assert!(event.pushed.is_none());
        assert_eq!(event.conversion_type.as_ref().unwrap().chars().count(), 80);
        assert_eq!(event.location.start, 0);
        assert_eq!(event.location.end, 1);
    }
}
