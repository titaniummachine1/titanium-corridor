import { AppController } from './game/appController.js';
import { renderBoard } from './ui/boardView.js';
import { renderControls } from './ui/controlsView.js';
import { renderEvalBar } from './ui/evalBar.js';
import { renderGameFooter } from './ui/gameFooter.js';

const appRoot = document.getElementById('app');
const controller = new AppController();

appRoot.innerHTML = `
  <div class="layout">
    <main class="layout__board" id="board-root">
      <div class="board-column">
        <div class="board-row">
          <aside class="board-row__eval" id="eval-root"></aside>
          <div class="board-row__grid" id="board-slot"></div>
        </div>
        <footer class="game-footer" id="game-footer"></footer>
      </div>
    </main>
    <aside class="layout__controls" id="controls-root"></aside>
  </div>
`;

const boardSlot = document.getElementById('board-slot');
const controlsRoot = document.getElementById('controls-root');
const evalRoot = document.getElementById('eval-root');
const footerRoot = document.getElementById('game-footer');

function renderBoardArea() {
  const state = controller.getState();
  renderEvalBar(evalRoot, state);
  renderBoard(boardSlot, state, controller);
  renderGameFooter(footerRoot, state);
}

function render() {
  renderBoardArea();
  renderControls(controlsRoot, controller.getState(), controller);
}

controller.onChange = render;
controller.onLiveUpdate = renderBoardArea;
render();
controller.maybeRequestAiMove();
