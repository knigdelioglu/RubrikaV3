import React from 'react';
import { useAppContext } from '../context/AppContext';
import { ProjectsPage } from './Projects';
import { NewProjectPage } from './NewProject';
import { WorkflowPage } from './Workflow';
import { DocumentsPage } from './Documents';
import { QuestionControlPage } from './QuestionControl';
import { RubricControlPage } from './RubricControl';
import { ExamPackagePage } from './ExamPackage';
import { StudentGroupingPage } from './StudentGrouping';
import { CropTemplatePage } from './CropTemplate';
import { OCRControlPage } from './OCRControl';
import { StudentIdentityPage } from './StudentIdentity';
import { ModelStatusPage } from './ModelStatus';
import { GradingReadyPage } from './GradingReady';

export function PageRouter() {
  const { currentPage } = useAppContext();

  switch (currentPage) {
    case 'projects': return <ProjectsPage />;
    case 'new_project': return <NewProjectPage />;
    case 'workflow': return <WorkflowPage />;
    case 'documents': return <DocumentsPage />;
    case 'question_control': return <QuestionControlPage />;
    case 'rubric_control': return <RubricControlPage />;
    case 'exam_package': return <ExamPackagePage />;
    case 'student_grouping': return <StudentGroupingPage />;
    case 'crop_template': return <CropTemplatePage />;
    case 'ocr_control': return <OCRControlPage />;
    case 'student_identity': return <StudentIdentityPage />;
    case 'model_status': return <ModelStatusPage />;
    case 'grading_ready': return <GradingReadyPage />;
    default: return <div>Sayfa bulunamadı</div>;
  }
}
