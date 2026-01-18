export interface AggregationResult {
  label: string;
  value: number;
  tech_stack?: string;
  category?: string;
}

export interface ViewResponse {
  view_id: string;
  items: AggregationResult[];
}

const MOCK_DATA: Record<string, ViewResponse> = {
  "top_file_size": {
    view_id: "top_file_size",
    items: [
      { label: "modules/configuration/gsrc/gw/api/graph/GraphTraversal.gs", value: 1250, tech_stack: "Gosu", category: "core" },
      { label: "modules/configuration/gsrc/gw/plugin/policy/impl/PolicyPlugin.gs", value: 980, tech_stack: "Gosu", category: "core" },
      { label: "modules/configuration/gsrc/gw/util/DateUtil.gs", value: 850, tech_stack: "Gosu", category: "util" },
      { label: "modules/configuration/gsrc/gw/api/financials/CurrencyAmount.gs", value: 720, tech_stack: "Gosu", category: "financials" },
      { label: "modules/configuration/gsrc/gw/api/domain/Claim.gs", value: 650, tech_stack: "Gosu", category: "mm" },
      { label: "modules/configuration/gsrc/gw/api/contact/Address.gs", value: 540, tech_stack: "Gosu", category: "mm" },
      { label: "modules/configuration/gsrc/gw/plugin/claim/ClaimSystemPlugin.gs", value: 430, tech_stack: "Gosu", category: "plugin" },
      { label: "modules/configuration/gsrc/gw/api/archiving/ArchivingUtil.gs", value: 410, tech_stack: "Gosu", category: "util" },
      { label: "modules/configuration/gsrc/gw/api/messaging/MessageSink.gs", value: 390, tech_stack: "Gosu", category: "messaging" },
      { label: "modules/configuration/gsrc/gw/api/webservice/cc/CCWsdlConfig.gs", value: 380, tech_stack: "Gosu", category: "webservice" },
    ]
  },
  "top_complexity": {
    view_id: "top_complexity",
    items: [
      { label: "modules/configuration/gsrc/gw/processes/BatchProcess.gs", value: 154, tech_stack: "Gosu", category: "maintainability" },
      { label: "modules/configuration/gsrc/gw/api/claim/ClaimUtil.gs", value: 120, tech_stack: "Gosu", category: "maintainability" },
      { label: "modules/configuration/src/com/guidewire/pl/system/bundle/Bundle.java", value: 98, tech_stack: "Java", category: "maintainability" },
      { label: "modules/configuration/gsrc/gw/plugin/billing/BillingSystemPlugin.gs", value: 87, tech_stack: "Gosu", category: "maintainability" },
      { label: "modules/configuration/gsrc/gw/api/validation/ValidationUtil.gs", value: 85, tech_stack: "Gosu", category: "maintainability" },
      { label: "modules/configuration/gsrc/gw/api/database/Query.gs", value: 76, tech_stack: "Gosu", category: "maintainability" },
      { label: "modules/configuration/gsrc/gw/api/util/StringUtil.gs", value: 65, tech_stack: "Gosu", category: "maintainability" },
      { label: "modules/configuration/gsrc/gw/plugin/document/DocumentProductionImpl.gs", value: 54, tech_stack: "Gosu", category: "maintainability" },
      { label: "modules/configuration/gsrc/gw/api/exposure/ExposureUtil.gs", value: 52, tech_stack: "Gosu", category: "maintainability" },
      { label: "modules/configuration/gsrc/gw/api/note/NoteUtil.gs", value: 48, tech_stack: "Gosu", category: "maintainability" },
    ]
  }
};

export async function fetchView(projectId: number | string, scanId: number | string, viewId: string): Promise<ViewResponse> {
  // In integration, we would fetch. 
  // const res = await fetch(`/api/v1/projects/${projectId}/scans/${scanId}/views/${viewId}`);
  // return res.json();
  console.log(`Fetching view ${viewId} for project ${projectId} scan ${scanId}`);

  // Simulate network delay
  await new Promise(r => setTimeout(r, 600));

  // Return mock for now as server isn't running in this environment context easily with data
  return MOCK_DATA[viewId] || { view_id: viewId, items: [] };
}
