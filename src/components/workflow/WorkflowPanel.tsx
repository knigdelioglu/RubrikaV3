import { useQuery } from "@tanstack/react-query";
import { Link } from "react-router-dom";
import { commands } from "../../api/commands";
import type { AssessmentActivity, JobSnapshot, ProjectSnapshot, WorkflowSnapshot } from "../../api/types";
import { useProjectContext } from "../../state/useProjectContext";
import { assessmentTypeLabels } from "../../pages/assessmentOrganizationUi";
import { resolveNextExamStep } from "../../app/examWorkspace";

type WorkflowPanelProps = {
  workflow?: WorkflowSnapshot;
  project?: ProjectSnapshot;
  jobs?: JobSnapshot[];
};

export function WorkflowPanel({ workflow }: WorkflowPanelProps) {
  const { projectId } = useProjectContext();

  const activitiesQuery = useQuery({
    queryKey: ["assessment-activities", projectId, "home"],
    queryFn: () => commands.listAssessmentActivities({ projectId }),
    enabled: !!projectId,
  });

  if (!workflow) return <div>Yükleniyor…</div>;

  const activities = activitiesQuery.data ?? [];

  if (activities.length === 0) {
    return (
      <div style={{ display: "grid", gap: "1.5rem", maxWidth: "720px", margin: "3rem auto 0" }}>
        <section style={{ padding: "3.5rem 2rem", background: "#fff", border: "1px dashed #cbd5e1", borderRadius: "1.25rem", textAlign: "center" }}>
          <h2 style={{ fontSize: "1.35rem", fontWeight: 800, color: "#0f172a", margin: 0 }}>Henüz sınav oluşturulmadı</h2>
          <p style={{ margin: "0.75rem 0 1.75rem", color: "#64748b", fontSize: "0.925rem", lineHeight: 1.55 }}>
            Yazılı, dinleme veya konuşma sınavı oluşturarak başlayın.
          </p>
          <Link to={`/project/${encodeURIComponent(projectId)}/activities`} className="button button--primary" style={{ padding: "0.75rem 1.75rem", fontSize: "0.95rem" }}>
            Yeni sınav oluştur
          </Link>
        </section>
      </div>
    );
  }

  return (
    <div style={{ display: "grid", gap: "1.5rem", maxWidth: "1180px", margin: "0 auto" }}>
      <section aria-label="Devam eden sınavlar">
        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: "0.9rem" }}>
          <div>
            <h2 style={{ margin: 0, fontSize: "1.05rem" }}>Sınavlar</h2>
            <p style={{ margin: "0.25rem 0 0", color: "#64748b", fontSize: "0.8rem" }}>Ders alanındaki ortak sınavlar ve sınıf uygulamaları.</p>
          </div>
          <Link to={`/project/${encodeURIComponent(projectId)}/activities`} className="button button--secondary" style={{ fontSize: "0.85rem" }}>
            Tüm sınavları yönet
          </Link>
        </div>
        <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(320px, 1fr))", gap: "1rem" }}>
          {activities.map((activity: AssessmentActivity) => {
            const applications = activity.classApplications.filter((app) => app.status !== "archived");
            const typeLabel = assessmentTypeLabels[activity.assessmentType] ?? activity.assessmentType;
            const nextStep = resolveNextExamStep(activity, workflow);
            const continuePath = `/project/${encodeURIComponent(projectId)}/activities/${encodeURIComponent(activity.id)}/${nextStep.id}`;

            return (
              <article key={activity.id} style={{ padding: "1.25rem", background: "#fff", border: "1px solid #e2e8f0", borderRadius: "0.9rem", display: "flex", flexDirection: "column", justifyContent: "space-between", gap: "0.85rem" }}>
                <div>
                  <div style={{ display: "flex", justifyContent: "space-between", alignItems: "flex-start", gap: "0.5rem" }}>
                    <h3 style={{ margin: 0, fontSize: "1rem", fontWeight: 700, color: "#0f172a" }}>
                      {activity.title || `${activity.term}. Dönem ${activity.sequenceNumber}. ${typeLabel}`}
                    </h3>
                    <span style={{ fontSize: "0.7rem", fontWeight: 700, padding: "0.2rem 0.5rem", background: "#e0e7ff", color: "#3730a3", borderRadius: "0.5rem", whiteSpace: "nowrap" }}>
                      {typeLabel}
                    </span>
                  </div>
                  <p style={{ margin: "0.4rem 0 0", color: "#64748b", fontSize: "0.8rem" }}>
                    {applications.length > 0
                      ? `Sınıflar: ${applications.map((app) => app.schoolClassId).join(", ")}`
                      : "Sınıf uygulaması yok"}
                  </p>
                  <div style={{ marginTop: "0.65rem", padding: "0.6rem 0.75rem", background: "#f8fafc", borderRadius: "0.6rem", border: "1px solid #f1f5f9" }}>
                    <span style={{ fontSize: "0.72rem", color: "#64748b", fontWeight: 600, display: "block" }}>Sıradaki işlem</span>
                    <strong style={{ fontSize: "0.85rem", color: "#1e293b" }}>{nextStep.label}</strong>
                  </div>
                </div>
                <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", borderTop: "1px solid #f1f5f9", paddingTop: "0.75rem" }}>
                  <span style={{ fontSize: "0.75rem", color: "#475569", fontWeight: 600 }}>
                    {activity.status === "completed" ? "Tamamlandı" : "Devam ediyor"}
                  </span>
                  <Link to={continuePath} className="button button--secondary" style={{ padding: "0.4rem 0.85rem", fontSize: "0.8rem" }}>
                    Devam et →
                  </Link>
                </div>
              </article>
            );
          })}
        </div>
      </section>
    </div>
  );
}
