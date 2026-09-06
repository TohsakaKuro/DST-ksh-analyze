import * as monaco from 'monaco-editor/esm/vs/editor/edcore.main';
import EditorWorker from 'monaco-editor/esm/vs/editor/editor.worker?worker';

let initialized = false;

export function initializeGlslEditor() {
  if (initialized) return monaco;

  globalThis.MonacoEnvironment = {
    getWorker() {
      return new EditorWorker();
    },
  };

  monaco.languages.register({ id: 'glsl' });
  monaco.languages.setLanguageConfiguration('glsl', {
    comments: { lineComment: '//', blockComment: ['/*', '*/'] },
    brackets: [['{', '}'], ['[', ']'], ['(', ')']],
    autoClosingPairs: [
      { open: '{', close: '}' },
      { open: '[', close: ']' },
      { open: '(', close: ')' },
    ],
  });
  monaco.languages.setMonarchTokensProvider('glsl', {
    tokenizer: {
      root: [
        [/\/\*/, 'comment', '@comment'],
        [/\/\/.*$/, 'comment'],
        [/#\w+/, 'preprocessor'],
        [/\b(attribute|const|uniform|varying|buffer|shared|coherent|volatile|restrict|readonly|writeonly|atomic_uint|break|continue|do|for|while|if|else|in|out|inout|float|int|void|bool|true|false|invariant|precise|discard|return|mat2|mat2x2|mat2x3|mat2x4|mat3|mat3x2|mat3x3|mat3x4|mat4|mat4x2|mat4x3|mat4x4|vec2|vec3|vec4|ivec2|ivec3|ivec4|bvec2|bvec3|bvec4|uvec2|uvec3|uvec4|lowp|mediump|highp|precision|sampler1D|sampler2D|sampler3D|samplerCube|struct)\b/, 'keyword'],
        [/0[xX][0-9a-fA-F]+[uU]?/, 'number'],
        [/(?:\d+\.?\d*|\.\d+)(?:[eE][+-]?\d+)?[fFuU]?/, 'number'],
        [/[=<>!]=|\+\+|--|\|\||&&|[+\-*/%<>=!]/, 'operator'],
        [/[a-zA-Z_]\w*/, 'identifier'],
      ],
      comment: [
        [/[^/*]+/, 'comment'],
        [/\*\//, 'comment', '@pop'],
        [/[/*]/, 'comment'],
      ],
    },
  });

  initialized = true;
  return monaco;
}
