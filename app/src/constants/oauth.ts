// OAuth Configuration — Aegis OS
// Client IDs registrados por el autor del proyecto.
// Estos IDs son públicos por diseño — el flujo PKCE no requiere Client Secret.
//
// Google: console.cloud.google.com — proyecto "Aegis OS"
//   Web Client ID:  para Expo Go durante desarrollo
//   iOS Client ID:  para builds nativas iOS
//
// Spotify: developer.spotify.com — app "Aegis OS"

export const OAUTH_CONFIG = {
  google: {
    // Para Expo Go y web: usar el Web Client ID
    clientId: '201101395662-v13ic0mv07drv8dvrucaos6kkqaaps0i.apps.googleusercontent.com',
    // Para builds nativas iOS
    iosClientId: '201101395662-an905drm5aqqog3sae5qp6ghq97o8vpi.apps.googleusercontent.com',
    // Para builds nativas Android (agregar cuando se tenga el SHA-1)
    androidClientId: '',
    scopes: [
      'https://www.googleapis.com/auth/youtube.readonly',
      'https://www.googleapis.com/auth/calendar.readonly',
      'https://www.googleapis.com/auth/drive.readonly',
      'https://www.googleapis.com/auth/gmail.readonly',
      'email',
      'profile',
    ],
    authorizationEndpoint: 'https://accounts.google.com/o/oauth2/v2/auth',
    tokenEndpoint: 'https://oauth2.googleapis.com/token',
  },
  spotify: {
    clientId: '3b1ff5d1f6d04fb3af5bdb1489062644',
    scopes: [
      'user-read-playback-state',
      'user-modify-playback-state',
      'user-read-currently-playing',
      'streaming',
      'playlist-read-private',
    ],
    authorizationEndpoint: 'https://accounts.spotify.com/authorize',
    tokenEndpoint: 'https://accounts.spotify.com/api/token',
  },
} as const;
