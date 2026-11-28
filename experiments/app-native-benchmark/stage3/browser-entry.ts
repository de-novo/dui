import { createDomModelRenderer, decodeModel, type Model, type ModelRenderContext } from "../../../apps/canvas/src/features/model/modelDocument";

const renderer = createDomModelRenderer();
declare global {
  interface Window {
    duiStage3?: {
      render(model: unknown, component: string, context: ModelRenderContext): unknown;
    };
  }
}
window.duiStage3 = {
  render(model: unknown, component: string, context: ModelRenderContext) {
    const decoded: Model = decodeModel(model);
    const result = renderer.render(decoded, component, context);
    return {
      html: result.html,
      markup: result.markup,
      viewport: result.viewport,
      unresolved: result.unresolved,
    };
  },
};
