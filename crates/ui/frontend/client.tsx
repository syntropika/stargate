import { createRoot } from 'react-dom/client';
import { App } from './app';
import './panel.css';

const props = JSON.parse(document.getElementById('stargate-config')!.textContent!);
createRoot(document.getElementById('stargate-root')!).render(<App {...props} />);
