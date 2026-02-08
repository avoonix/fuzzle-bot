import { createRouter, createWebHistory } from 'vue-router'
import HomeView from '../views/HomeView.vue'
import BanSetView from '@/views/BanSetView.vue'
import BanSimilarView from '@/views/BanSimilarView.vue'
import BannedStickersView from '@/views/BannedStickersView.vue'

const router = createRouter({
  history: createWebHistory(import.meta.env.BASE_URL),
  routes: [
    {
      path: '/',
      name: 'home',
      component: HomeView,
    },
    {
      path: '/banned-stickers',
      name: 'bannedStickers',
      component: BannedStickersView,
    },
    {
      path: '/bans',
      name: 'bans',
      component: HomeView,
      children: [
        {
          path: 'set/:setId',
          component: BanSetView,
          name: 'banSetView',
          children: [
            {
          path: 'sticker/:stickerId/similar',
          component: BanSimilarView,
          name: 'banSimilarView',
            }
          ]
        },
      ],
    },
    {
      path: '/about',
      name: 'about',
      // route level code-splitting
      // this generates a separate chunk (About.[hash].js) for this route
      // which is lazy-loaded when the route is visited.
      component: () => import('../views/AboutView.vue'),
    },
  ],
})

export default router
