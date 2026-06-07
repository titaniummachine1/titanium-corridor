import { AppController } from './game/appController.js';
import { renderBoard } from './ui/boardView.js';
import { renderControls } from './ui/controlsView.js';
import { renderEvalBar } from './ui/evalBar.js';

const appRoot = document.getElementById('app');
const controller = new AppController();

appRoot.innerHTML = `
  <div class="layout">
    <aside class="layout__eval" id="eval-root"></aside>
    <main class="layout__board" id="board-root"></main>
    <aside class="layout__controls" id="controls-root"></aside>
  </div>
`;

const boardRoot = document.getElementById('board-root');
const controlsRoot = document.getElementById('controls-root');
const evalRoot = document.getElementById('eval-root');

function render() {
  const state = controller.getState();
  renderEvalBar(evalRoot, state);
  renderBoard(boardRoot, state, controller);
  renderControls(controlsRoot, state, controller);
}

controller.onChange = render;
render();
controller.maybeRequestAiMove();
