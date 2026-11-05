import Foundation
import MediaMenuCore

struct IngestDraft {
    let source: String, kind: String, project: String, profile: String, country: String, city: String
    let archive: String, work: String, copy: String, loadedKey: String
    let cleanup: Bool, deferWork: Bool, archiveException: Bool
    let units: [DraftUnit]
    let reviewed: ReviewedPlan?
}

extension Model {
    func parkIngestDraft() {
        guard !busy, lastResult == nil, !source.isEmpty, !units.isEmpty || !project.isEmpty || plan != nil else { return }
        ingestDrafts[source] = IngestDraft(source: source, kind: sourceKind, project: project, profile: profile,
            country: country, city: city, archive: archive, work: work, copy: copyChoice, loadedKey: loadedSourceKey,
            cleanup: cleanup, deferWork: deferWork, archiveException: archiveExceptionConfirmed, units: units, reviewed: reviewedPlan)
    }
    func restoreIngestDraft(_ path: String) {
        guard !busy, let saved = ingestDrafts[path] else { return }
        parkIngestDraft(); ingestDrafts.removeValue(forKey: path)
        sourceKind = saved.kind; source = saved.source; project = saved.project; profile = saved.profile
        country = saved.country; city = saved.city; archive = saved.archive; work = saved.work
        copyChoice = saved.copy; cleanup = saved.cleanup; deferWork = saved.deferWork
        archiveExceptionConfirmed = saved.archiveException
        units = saved.units; loadedSourceKey = saved.loadedKey; lastResult = nil
        plan = saved.reviewed?.display; reviewedPlan = saved.reviewed
        route = .ingest; ingestStep = .content
    }
}
